#[macro_use]
extern crate serde_json;
extern crate hyper;
pub mod indexing;
pub mod parsing;
pub mod schema;
pub mod utility;

use futures::future;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use std::collections::HashMap;
type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;

use graphql_parser::parse_query;
use serde_json::Value;
extern crate serde;
use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub config: ConfigConfig,
    pub database: Vec<ConfigDatabase>,
}

#[derive(Deserialize)]
pub struct ConfigConfig {
    pub instropection: String,
    pub canonical: String,
}

#[derive(Deserialize)]
pub struct ConfigDatabase {
    pub name: String,
}

#[derive(Clone)]
struct App {
    parser: HashMap<String, parsing::QueryParser>,
    statics: HashMap<String, String>,
}
impl App {
    pub fn new() -> App {
        let files = vec!["graphiql.html", "index.html"];
        let mut filess = HashMap::new();
        for file in files {
            filess.insert(file.to_owned(), utility::read_pub_file(&file));
        }
        let config : Config = toml::from_str(&utility::read_db_file("config.toml")[..]).expect("config.toml is not valid!");

        let dbs =  config.database.iter().map(|db| (db.name.clone(),
        parsing::QueryParser::new(parsing::read_database(&db.name[..]), schema::read_schema(&db.name[..])))
        ).collect::<HashMap<String, parsing::QueryParser>>();
        App {
            parser: dbs,
            statics: filess,
        }
    }

    fn graphql_api(&self, req: Request<Body>) -> ResponseFuture {
        // A web api to run against
        let urlpa = url::Url::parse("https://example.com").unwrap().join(&req.uri().to_string()).unwrap();
        let mut db = urlpa.query_pairs();
        let dbb = db.find(|(k,_)| k=="db").unwrap_or((std::borrow::Cow::Borrowed("db"),std::borrow::Cow::Borrowed( ""))).1;
        let parser = self.parser[&dbb[..]].clone();
        Box::new(
            req.into_body()
                .concat2() // Concatenate all chunks in the body
                .from_err()
                .and_then(|entire_body| {
                    // TODO: Replace all unwraps with proper error handling
                    let parser = parser;
                    let str = String::from_utf8(entire_body.to_vec())?;
                    let data: serde_json::Value = serde_json::from_str(&str)?;
                    let query = match &data["query"] {
                        Value::String(query) => query,
                        _ => "{}",
                    };
                    let freee = serde_json::Map::default();
                    let vars = match &data["variables"] {
                        Value::Object(query) => query,
                        _ => &freee,
                    };

                    match parse_query(&query) {
                        Ok(v) => {
                            let values = parser.traverse_query(&v, &vars);
                            let data = json!({ "data": values });
                            Ok(Response::builder()
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(data.to_string()))
                                .unwrap())
                        }
                        _ => Ok(Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(
                                json!({"data":null, "error":"Invalid GraphQL syntax"}).to_string(),
                            ))
                            .unwrap()),
                    }
                }),
        )
    }

    fn serve_static(&self, path: &str, content_type: &str) -> ResponseFuture {
        Box::new(future::ok(
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(self.statics[path].clone()))
                .unwrap(),
        ))
    }

    pub fn app_worker(&self, req: Request<Body>) -> ResponseFuture {
        match (req.method(), req.uri().path()) {
            (&Method::POST, "/graphql") => self.graphql_api(req),
            (&Method::GET, "/graphiql") => self.serve_static("graphiql.html", "text/html"),
            (&Method::GET, "/") => self.serve_static("index.html", "text/html"),
            _ => Box::new(future::ok(
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(""))
                    .unwrap(),
            )),
        }
    }
}

fn main() {
    let addr = ([127, 0, 0, 1], 3000).into();
    let app = App::new();
    let server = Server::bind(&addr)
        .serve(move || {
            let app = app.clone();
            service_fn(move |x| app.app_worker(x))
        })
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);
    hyper::rt::run(server);
}
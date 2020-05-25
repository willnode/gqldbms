#[macro_use]
extern crate serde_json;
extern crate hyper;
extern crate serde;

pub mod canonical;
pub mod indexing;
pub mod parsing;
pub mod resolver;
pub mod schema;
pub mod structure;
pub mod utility;

use futures::future;
use graphql_parser::parse_query;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use serde_derive::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;

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
    parser: parsing::DatabaseDirectory,
    statics: HashMap<String, String>,
}

impl App {
    pub fn new() -> App {
        let files = vec!["graphiql.html", "index.html"];
        let mut filess = HashMap::new();
        for file in files {
            filess.insert(file.to_owned(), utility::read_pub_file(&file));
        }
        let config: Config = toml::from_str(&utility::read_db_file("config.toml")[..])
            .expect("config.toml is not valid!");

        let dbs = Arc::new(RwLock::new(HashMap::new()));
        for db in &config.database {
            let dbss = dbs.clone();
            let dddd = &mut *dbs.write().unwrap();
            println!("Loading {}", db.name);
            dddd.insert(db.name.clone(), utility::load_db(&db.name[..], dbss));
        }
        {
            println!("Loading canonical");
            let dbss = dbs.clone();
            let canonical = utility::load_canonical(dbss);
            (*dbs.write().unwrap()).insert("canonical".to_owned(), canonical);
        }
        println!("Ready...");
        App {
            parser: dbs,
            statics: filess,
        }
    }

    fn graphql_api(&self, req: Request<Body>) -> ResponseFuture {
        // A web api to run against
        let urlpa = url::Url::parse("https://example.com")
            .unwrap()
            .join(&req.uri().to_string())
            .unwrap();
        let mut db = urlpa.query_pairs();
        let dbb = match &db.find(|(k, _)| k == "db") {
            Some(v) => (&v.1).to_string(),
            _ => "".to_owned(),
        };
        let parser = self.parser.clone();
        // let parser = self.parser.get(&dbb[..]);
        // let parser = match parser {
        //     Some(v) => v,
        //     _ => {
        //         return Box::new(future::ok(
        //             Response::builder()
        //                 .status(StatusCode::OK)
        //                 .body(Body::from(
        //                     json!({"data":null, "error":"Database entry not found"}).to_string(),
        //                 ))
        //                 .unwrap(),
        //         ));
        //     }
        // };
        // let parser = parser;

        Box::new(
            req.into_body()
                .concat2() // Concatenate all chunks in the body
                .from_err()
                .and_then(move |entire_body| {
                    // TODO: Replace all unwraps with proper error handling
                    let parser2 = &*parser.read().unwrap_or_else(|e| e.into_inner());
                    let parser3 = match parser2.get(&dbb[..]) {
                        Some(v) => v,
                        _ => {
                            return Ok(Response::builder()
                                .status(StatusCode::OK)
                                .body(Body::from(
                                    json!({"data":null, "error":"Database entry not found"})
                                        .to_string(),
                                ))
                                .unwrap())
                        }
                    };
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
                            let values = parser3.traverse_query(&v, &vars);
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

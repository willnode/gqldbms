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

#[derive(Clone)]
struct App {
    parser: parsing::QueryParser,
    statics: HashMap<String, String>,
}
impl App {
    pub fn new() -> App {
        let files = vec!["graphiql.html", "instropection.gql", "schema.gql", "index.html"];
        let mut filess = HashMap::new();
        for file in files {
            filess.insert(file.to_owned(), utility::read_pub_file(&file));
        }
        App {
            parser: parsing::QueryParser::new(),
            statics: filess,
        }
    }

    fn graphql_api(&self, req: Request<Body>) -> ResponseFuture {
        // A web api to run against
        let parser = self.parser.clone();
        Box::new(
            req.into_body()
                .concat2() // Concatenate all chunks in the body
                .from_err()
                .and_then(|entire_body| {
                    // TODO: Replace all unwraps with proper error handling
                    let parser = parser;
                    let str = String::from_utf8(entire_body.to_vec())?;
                    let data: serde_json::Value = serde_json::from_str(&str)?;
                    let mut json = String::new();
                    match &data["query"] {
                        Value::String(query) => json.push_str(&query),
                        _ => json.push_str("{}"),
                    };
                    match parse_query(&json) {
                        Ok(v) => {
                            let values = parser.traverse_query(&v);
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

#[macro_use]
extern crate serde_json;
extern crate hyper;
pub mod parsing;
pub mod schema;

use futures::future;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Server, StatusCode, header};
type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item=Response<Body>, Error=GenericError> + Send>;

use std::fs::File;
use std::io::Read;
use std::str;
use serde_json::Value;
use graphql_parser::parse_query;

fn read_file(file: &str) -> String {
    let mut uri = String::from("public/");
    uri.push_str(file);
    let mut file = File::open(uri).expect("Unable to open");
    let mut data = String::new();
    file.read_to_string(&mut data).expect("Empty");
    data
}

fn graphql_api(req: Request<Body>) -> ResponseFuture {
    // A web api to run against
    Box::new(req.into_body()
        .concat2() // Concatenate all chunks in the body
        .from_err()
        .and_then(|entire_body| {
            // TODO: Replace all unwraps with proper error handling
            let str = String::from_utf8(entire_body.to_vec())?;
            let data : serde_json::Value = serde_json::from_str(&str)?;
            let mut json = String::new();
            match &data["query"] {
                Value::String(query) => json.push_str(&query),
                _ => json.push_str("{}"),
            }
            let ast = match parse_query(&json) {
                Ok(v) => v,
                _ => return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"error":"Invalid GraphQL syntax"}).to_string()))?)
            };
            let values = parsing::traverse_query(&ast);
            let data = json!({ "data": values });
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(data.to_string()))?;
            Ok(response)
        })
    )
}

fn echo(req: Request<Body>) -> ResponseFuture {
    let mut response = Response::new(Body::empty());

    match (req.method(), req.uri().path()) {
        (&Method::GET, "/data") => {
            response.headers_mut().insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
            *response.body_mut() = Body::from(read_file("data.json"));
        }
        (&Method::GET, "/graphiql") => {
            *response.body_mut() = Body::from(read_file("graphiql.html"));
        }
        (&Method::GET, "/schema") => {
            *response.body_mut() = Body::from(read_file("schema.gql"));
        }
        (&Method::POST, "/graphql") => {
           return  graphql_api(req)
        }

        // The 404 Not Found route...
        _ => {
            *response.status_mut() = StatusCode::NOT_FOUND;
        }
    };

    Box::new(future::ok(response))
}

fn main() {
    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn(echo))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);
    hyper::rt::run(server);
}
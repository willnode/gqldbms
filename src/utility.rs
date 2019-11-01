use std::fs::File;
use std::io::Read;
use graphql_parser::query::Value as GraphValue;
use serde_json::Value as JSONValue;

pub fn read_pub_file(path: &str) -> String {
	let uri = String::from("public/") + path;
	let mut file = File::open(uri).expect(&format!("Unable to open `public/{}`", path)[..]);
	let mut data = String::new();
	file.read_to_string(&mut data).expect(&format!("Unable to read `public/{}` (Invalid UTF-8 file?)", path)[..]);
	data
}

pub fn read_db_file(path: &str) -> String {
	let uri = String::from("database/") + path;
	let mut file = File::open(uri).expect(&format!("Unable to open `database/{}`", path)[..]);
	let mut data = String::new();
	file.read_to_string(&mut data).expect(&format!("Unable to read `database/{}` (Invalid UTF-8 file?)", path)[..]);
	data
}


pub fn gql2serde_value(v: &GraphValue) -> JSONValue {
	match v {
		GraphValue::Boolean(b) => json!(b),
		GraphValue::Int(i) => json!(i.as_i64()),
		GraphValue::Float(f) => json!(f),
		GraphValue::String(s) => json!(s),
		GraphValue::Variable(s) => json!(s),
		_ => json!(null),
	}
}
use super::{parsing, schema, structure};
use graphql_parser::query::Value as GraphValue;
use serde_json::Value as JSONValue;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::collections::HashMap;

pub fn read_file(uri: &str) -> String {
	let mut file = File::open(uri).expect(&format!("Unable to open `{}`", uri)[..]);
	let mut data = String::new();
	file.read_to_string(&mut data)
		.expect(&format!("Unable to read `{}` (Invalid UTF-8 file?)", uri)[..]);
	data
}

pub fn read_pub_file(path: &str) -> String {
	let uri = String::from("public/") + path;
	let mut file = File::open(uri).expect(&format!("Unable to open `public/{}`", path)[..]);
	let mut data = String::new();
	file.read_to_string(&mut data)
		.expect(&format!("Unable to read `public/{}` (Invalid UTF-8 file?)", path)[..]);
	data
}

pub fn read_db_file(path: &str) -> String {
	let uri = String::from("database/") + path;
	let mut file = File::open(uri).expect(&format!("Unable to open `database/{}`", path)[..]);
	let mut data = String::new();
	file.read_to_string(&mut data)
		.expect(&format!("Unable to read `database/{}` (Invalid UTF-8 file?)", path)[..]);
	data
}

fn write_file(uri: &str, text: Vec<u8>) {
	let mut file = File::create(uri.clone()).expect(&format!("Unable to open `database/{}`", uri)[..]);
	file.write_all(&text)
		.expect(&format!("Unable to read `database/{}` (Invalid UTF-8 file?)", uri)[..]);
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

fn read_database(db: &str) -> parsing::DatabaseIndex {
	let data = read_file(db);
	serde_json::from_str(&data).expect("File `database/data.json` is not valid JSON object!")
}

fn read_structure(db: &str) -> structure::StructureIndex {
	let data = read_file(db);
	serde_json::from_str::<structure::StructureIndex>(&data)
		.expect("File `database/structure.json` is not valid JSON object!")
}

fn read_schema(path: &str) -> graphql_parser::schema::Document {
	let data = read_file(path);
	graphql_parser::parse_schema(&data)
		.expect(&format!("File `database/{}` is not valid GraphQL schema!", path)[..])
}

pub fn load_db(name: &str) -> parsing::QueryParser {
	let (json_path, schema_path, gql_path, instropection_path) = (
		format!("database/{}/data.json", name),
		format!("database/{}/schema.json", name),
		format!("database/{}/schema.gql", name),
		format!("database/instropection.gql"),
	);

	let db = read_database(json_path.as_ref());
	let sch = if std::fs::metadata(schema_path.clone()).is_ok() {
		read_structure(schema_path.as_ref()).into_perform_indexing()
	} else {
		let sch = schema::traverse_schema(&read_schema(gql_path.as_ref()));
		write_file(schema_path.as_ref(), json!(sch).to_string().as_bytes().to_vec());
		sch
	};
	let intros = schema::traverse_schema(&read_schema(instropection_path.as_ref()));
	parsing::QueryParser::new(db, sch, intros)
}

pub fn load_canonical(all_db: &HashMap<String, parsing::QueryParser>) -> parsing::QueryParser {
	let (gql_path, instropection_path) = (
		format!("database/canonical.gql"),
		format!("database/instropection.gql")
	);
	let mut databases = Vec::new();
	let mut objects = Vec::new();
	// let mut fields = Vec::new();

	for (key, val) in all_db {
		for valt in &val.schema.objects {
			objects.push(json!({
				"id": format!("{}.{}", key, valt.name),
				"name": valt.name.clone(),
				"description": valt.description.clone()
			}))
		}
		databases.push(json!({
			"id": key.clone(),
			"name": key.clone(),
			"types": [],
			"enums": [],
		}))
	}
	let db : HashMap<String, Vec<JSONValue>> = [
		("Database".to_owned(), databases),
		("Object".to_owned(), objects),
		("Query".to_owned(), vec![json!({
			"databases": []
		})])
	].iter().cloned().collect();
	let sch =  schema::traverse_schema(&read_schema(gql_path.as_ref()));
	// write_file(schema_path.as_ref(), json!(sch).to_string().as_bytes().to_vec());
	let intros = schema::traverse_schema(&read_schema(instropection_path.as_ref()));
	let mut res = parsing::QueryParser::new(db, sch, intros);
	res.is_canonical = true;
	res
}
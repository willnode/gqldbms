use serde_json::Value;
use graphql_parser::query::*;


use super::schema;
use std::fs::File;
use std::io::Read;
use std::collections::HashMap;

struct QueryParser {
	pub schema: schema::SchemaClasses,
	pub database: Value,
}

struct ResolverInfo {
	class_name: String,
	field_name: String,
	field_type: schema::FieldType,
}

struct TraverserInfo {
	class_name: String,

}

fn database() -> Value
{
	let uri = String::from("public/data.json");
    let mut file = File::open(uri).expect("Unable to open");
    let mut data = String::new();
    file.read_to_string(&mut data).expect("Empty");
    serde_json::from_str(&data).unwrap()
}

impl QueryParser {

pub fn resolve_field(&self, parent:&Value, _args:&Vec<(String, graphql_parser::query::Value)>, context:&Field, info:ResolverInfo) -> Value
{
	match parent {
		Value::Null => {
			// Global fields on Query
			if !info.field_type.is_array {
				// TODO: Look into args
				json!(null)
			} else {
				// TODO: Look into args for filters
				let ids = match &self.database[&info.class_name] {
					Value::Array(arr) => arr.iter().map(|x| { &x["id"] }).collect(),
					_ => Vec::new(),
				};
				self.traverse_selection(&json!(ids), &context.selection_set, TraverserInfo {
						class_name: info.field_type.name_type
					})
			}
		}
		_ => {
			if !info.field_type.is_array {
				parent[&info.field_name].clone()
			} else {
				match parent[&info.field_name] {
					// TODO: Look into args for filters
					Value::Array(_) => self.traverse_selection(&parent[&info.field_name],
						&context.selection_set, TraverserInfo {
						class_name: info.field_type.name_type
					}),
					_ => json!(""),
				}
			}
		}
	}
}

pub fn traverse_selection(&self, parent:&Value, context : &SelectionSet, info:TraverserInfo) -> Value
{
	let mut values = HashMap::new();

	match parent {
		Value::Null => {
			// Global query
			let sch = &self.schema[&info.class_name];
			for sel in &context.items {
				match &sel {
					Selection::Field(field) => {
						match &sch {
							schema::SchemaType::Object(fields) => {
								values.insert(field.name.clone(), self.resolve_field(
									&Value::Null, &field.arguments, &field, ResolverInfo {
										class_name: info.class_name.clone(),
										field_name: field.name.clone(),
										field_type: fields [&field.name].clone()
									}));
							}
						}
					}
					_ => {},
				};
			}
		}
		Value::Array(_) => {

		}
		_ => {

		}
	}

	json!(values)
}
}


pub fn traverse_query(ast : &Document) -> Value {
	let worker = QueryParser {
		schema: schema::traverse_schema(),
		database: database()
	};
	for def in &ast.definitions {
		match &def {
			Definition::Operation(opdef) => {
				let query_str = String::from("Query");
				match &opdef {
					OperationDefinition::Query(query) => {
						return worker.traverse_selection(&Value::Null, &query.selection_set, TraverserInfo {
							class_name: query_str
						});
					}
					OperationDefinition::SelectionSet(sel) => {
						return worker.traverse_selection(&Value::Null, &sel, TraverserInfo {
							class_name: query_str
						});
					}
					_ => return json!("Other op"),
				}
			},
			_ => return json!("Other def"),
		}
	}
	return json!("");
}

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

#[derive(Debug)]
struct ResolverInfo {
	field_name: String,
	field_type: schema::FieldType,
}

#[derive(Debug)]
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

fn gql2serde_value(v:&graphql_parser::query::Value) -> Value {
	match v {
		graphql_parser::query::Value::Boolean(b) => json!(b),
		graphql_parser::query::Value::Int(i) => json!(i.as_i64()),
		graphql_parser::query::Value::Float(f) => json!(f),
		graphql_parser::query::Value::String(s) => json!(s),
		_ => json!(null),
	}
}

impl QueryParser {

// Given Values, Apply filters to it
fn find_match(&self, class_name:&String, iter:Vec<&Value>, args:&Vec<(String, graphql_parser::query::Value)>) -> Value
{
	let mut result : Vec<Value> = iter.into_iter().map(|x|{
		match &self.database[&class_name] {
			Value::Array(arr) => {
				arr.into_iter().find(|y| {&y["id"] == x}).unwrap().clone()
			}
			_ => Value::Null.clone(),
		}
	}).collect();
	for (key, val) in args {
		let val2 = gql2serde_value(val);
		result = result.into_iter().filter(|x| {x[&key] == val2}).collect();
	}
	json!(result)
}

// Resolve/Expand JSON database to object representation (by looking their Schema Type)
fn resolve_id_to_object(&self, id:&Value, class_name:&String) -> Value
{
	match id {
			Value::Array(arr) => {
				// Unpack array and resolve individually
				json!(arr.iter().map(|x| self.resolve_id_to_object(x, &class_name)).collect::<Vec<Value>>())
			},
			x => {
				match &class_name[..] {
				// A primitive
				"String" | "ID" | "Number" | "Float" => id.clone(),
				// Object in schema
				_ => match &self.database[&class_name] {
					Value::Array(arr) => {
						// Unpack object
						let val = arr.iter().find(|y| {&y["id"] == x});
						match val {
							Some(v) => v.clone(),
							_ => json!(null),
						}
					},
					_ => panic!(),
				}
			}
		}
	}
}

pub fn resolve_field(&self, parent:&Value, args:&Vec<(String, graphql_parser::query::Value)>, context:&Field, info:ResolverInfo) -> Value
{
	match parent {
		Value::Null => {
			// Global fields on Query
			// TODO: Look into args for filters
			let ids = match &self.database[&info.field_type.name_type] {
				// Get all IDs
				Value::Array(arr) => arr.iter().map(|x| { &x["id"] }).collect(),
				_ => Vec::new(),
			};
			let matches = self.find_match(&info.field_type.name_type, ids, args);
			self.traverse_selection(&matches, &context.selection_set, TraverserInfo {
				class_name: info.field_type.name_type
			})
		}
		_ => {
			if parent[&info.field_name] == Value::Null {
				return Value::Null;
			}
			self.traverse_selection(
				&self.resolve_id_to_object(
					&parent[&info.field_name],
					&info.field_type.name_type
				),
				&context.selection_set, TraverserInfo {
				class_name: info.field_type.name_type
			})
		}
	}
}

pub fn traverse_selection(&self, parent:&Value, context : &SelectionSet, info:TraverserInfo) -> Value
{
	match parent {
		Value::Null => {
			// Global query
			let mut values = HashMap::new();
			let sch = &self.schema[&info.class_name];
			for sel in &context.items {
				match &sel {
					Selection::Field(field) => {
						match &sch {
							schema::SchemaType::Object(fields) => {
								if field.name == "__schema" {
									return json!({}); // Instropection Query, later on
								}
								values.insert(field.name.clone(), self.resolve_field(
									&Value::Null, &field.arguments, &field, ResolverInfo {
										field_name: field.name.clone(),
										field_type: fields [&field.name].clone()
									}));
							}
						}
					}
					_ => {},
				};
			}
			json!(values)
		}
		Value::Array(arr) => {
			if arr.len() == 0 {
				return json!([]); // Workaround for later bug
			}
			let mut values : Vec<HashMap<Name, Value>> = arr.iter().map(|_|{HashMap::new()}).collect();
			let sch = &self.schema[&info.class_name];
			for sel in &context.items {
				match &sel {
					// Field for parent
					Selection::Field(field) => {
						match &sch {
							schema::SchemaType::Object(fields) => {
								for (i, obj) in arr.iter().enumerate() {
									values[i].insert(field.name.clone(), self.resolve_field(
										obj, &field.arguments, &field, ResolverInfo {
										field_name: field.name.clone(),
										field_type: fields [&field.name].clone()
									}));
								}
							}
						}
					}
					_ => {},
				};
			}
			json!(values)
		}
		_ => {
			match &info.class_name[..] {
				"String" | "ID" | "Number" | "Float" => parent.clone(),
				_ => {
					let mut values : HashMap<Name, Value> = HashMap::new();
					let sch = &self.schema[&info.class_name];
					for sel in &context.items {
						match &sel {
							// Field for parent
							Selection::Field(field) => {
								match &sch {
									schema::SchemaType::Object(fields) => {
										{
											values.insert(field.name.clone(), self.resolve_field(
												&parent, &field.arguments, &field, ResolverInfo {
												field_name: field.name.clone(),
												field_type: fields [&field.name].clone()
											}));
										}
									}
								}
							}
							_ => {},
						};
					}
					json!(values)
		}
			}
		}
	}
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

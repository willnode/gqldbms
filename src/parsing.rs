use serde_json::Value;
use graphql_parser::query::*;

use std::convert::TryInto;
use super::schema;
use std::fs::File;
use std::io::Read;
use std::collections::HashMap;

struct QueryParser {
	pub schema: schema::SchemaClasses,
	pub database: Value,
	pub hashmaps: DatabaseHashmaps,
	pub instropection: schema::InstropectionParser,
}

#[derive(Debug)]
struct ResolverInfo<'a> {
	field_name: &'a str,
	field_type: schema::SchemaFieldReturnType,
	instropection: bool,
}

#[derive(Debug)]
struct TraverserInfo<'a> {
	class_name: &'a str,
	instropection: bool,
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
				n @ _ => match &self.database[&n] {
					Value::Array(arr) => {
						// Unpack object
						let idkey = match &self.hashmaps[&n[..]] { Some(v) => &v["id"], _ => panic!() };
						arr[match &idkey {
							FieldHashmaps::String(h) => h[&x.as_str().unwrap()[..]],
							FieldHashmaps::I32(h) => h[&x.as_i64().unwrap().try_into().unwrap()],
							FieldHashmaps::U64(h) => h[&x.as_u64().unwrap()],
							_ => panic!()
						}].clone()
					},
					n @ _ => {return json!(n);},
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
				class_name: &info.field_type.name_type[..],
				instropection: info.instropection,
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
				class_name: &info.field_type.name_type[..],
				instropection: info.instropection,
			})
		}
	}
}

pub fn traverse_selection(&self, parent:&Value, context : &SelectionSet, info:TraverserInfo) -> Value
{
	let using_schema = if info.instropection { &self.instropection.schema } else { &self.schema };
	match parent {
		Value::Null => {
			// Global query
			let mut values = HashMap::new();
			let sch = match using_schema.get(&info.class_name[..]) { Some(v) => v, _ => {return Value::Null;} };
			for sel in &context.items {
				match &sel {
					Selection::Field(field) => {
						match &sch {
							schema::SchemaType::Object(fields) => {
								if field.name == "__schema" {
								values.insert(field.name.clone(), self.traverse_selection(
									 	&Value::Null, &field.selection_set, TraverserInfo {
										class_name: "__Schema",
										instropection: true,
									}));
								} else if !fields.contains_key(&field.name) {
									values.insert(field.name.clone(), Value::Null);
								} else {

								values.insert(field.name.clone(), self.resolve_field(
									&Value::Null, &field.arguments, &field, ResolverInfo {
										field_name: &field.name[..],
										field_type: fields[&field.name].return_type.clone(),
										instropection: info.instropection,
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
		Value::Array(arr) => {
			if arr.len() == 0 {
				return json!([]); // Workaround for later bug
			}
			let mut values : Vec<HashMap<Name, Value>> = arr.iter().map(|_|{HashMap::new()}).collect();
			let sch = match using_schema.get(&info.class_name[..]) { Some(v) => v, _ => {return Value::Null;} };
			for sel in &context.items {
				match &sel {
					// Field for parent
					Selection::Field(field) => {
						match &sch {
							schema::SchemaType::Object(fields) => {
								for (i, obj) in arr.iter().enumerate() {
									values[i].insert(field.name.clone(), self.resolve_field(
										obj, &field.arguments, &field, ResolverInfo {
										field_name: &field.name[..],
										field_type: fields [&field.name].return_type.clone(),
										instropection: info.instropection,
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
					let sch = match using_schema.get(&info.class_name[..]) { Some(v) => v, _ => {return Value::Null;} };
					for sel in &context.items {
						match &sel {
							// Field for parent
							Selection::Field(field) => {
								match &sch {
									schema::SchemaType::Object(fields) => {
										{
											values.insert(field.name.clone(), self.resolve_field(
												&parent, &field.arguments, &field, ResolverInfo {
												field_name: &field.name[..],
												field_type: fields [&field.name].return_type.clone(),
												instropection: info.instropection,
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

pub enum FieldHashmaps {
	String(HashMap<String, usize>),
	Boolean(HashMap<bool, usize>),
	I32(HashMap<i32, usize>),
	U64(HashMap<u64, usize>),
	Null,
}

// Type, PropertyName (usually "id"), ValueOfProperty. returns index in database
// Valid for database lifetimes
pub type DatabaseHashmaps = HashMap<String, Option<HashMap<String, FieldHashmaps>>>;

fn subindex_hashmaps<T, F>(classes : &Vec<Value>, converter: F)
	-> HashMap<T, usize> where F: Fn(&Value) -> T, T: std::hash::Hash + Eq
{
	let mut h = HashMap::new();
	for (index, value) in classes.iter().enumerate() {
		h.insert(converter(&value), index);
	}
	h
}

pub fn build_hashmaps(db : &Value, schema : &schema::SchemaClasses) -> DatabaseHashmaps {
	let mut hashes : DatabaseHashmaps = HashMap::new();
	for (name, classes) in match &db { Value::Object(arr) => arr, _ => panic!("Database Root must be an Object!") } {
		if !schema.contains_key(name) {
			// No indexing needed, 'cause we can't infer exact type this way
			// Anyway if things doesn't exist in schema, it never be looked up, so don't panic.
			hashes.insert(name.clone(), Option::None);
		} else {
			// This type exist in schema. Let's index
			// WIP: Indexable props should be marked, but now let's just assume it's only ID for now.
			let arr_classes = match &classes { Value::Array(arr) => arr, _ => panic!("All Database classlist must be an Array!") };
			let (field_name, hash) = ("id".to_owned(), match &schema[name] {
				schema::SchemaType::Object(obj) => match obj.get("id") {
					Option::Some(field) => if !field.data_type.is_indexed {
						panic!("id must be indexable!")
					} else { match &field.data_type.name_type[..] {
						"string" => FieldHashmaps::String(subindex_hashmaps(arr_classes, |value| value["id"].as_str().unwrap().to_string())),
						"i32" => FieldHashmaps::I32(subindex_hashmaps(arr_classes, |value| value["id"].as_i64().unwrap().try_into().unwrap())),
						"u64" => FieldHashmaps::U64(subindex_hashmaps(arr_classes, |value| value["id"].as_u64().unwrap())),
						_ => panic!("id is not indexable!")
					} },
					_ => panic!("id is not exist in one of schema class!")
				},
				// _ => panic!("An object is exist in DB, but in schema it's refered as something else")
			});

			let mut type_hash = HashMap::new();
			type_hash.insert(field_name, hash);
			hashes.insert(name.clone(), Some(type_hash));
		}
	}
	hashes
}

pub fn traverse_query(ast : &Document) -> Value {
	let sch = schema::traverse_schema("schema.gql");
	let instropection = schema::build_schema_instropection();
	let mut db = database();
	let dbmut = match db.as_object_mut() { Some(o) => o, _ => panic!("Database must be object!") };
	dbmut.extend(instropection.database.as_object().unwrap().clone());
	db = json!(dbmut);

	let hashmap = build_hashmaps(&db, &sch);
	let worker = QueryParser {
		schema: sch,
		database: db,
		hashmaps: hashmap,
		instropection: instropection,
	};
	for def in &ast.definitions {
		match &def {
			Definition::Operation(opdef) => {
				match &opdef {
					OperationDefinition::Query(query) => {
						return worker.traverse_selection(&Value::Null, &query.selection_set, TraverserInfo {
							class_name: "Query",
							instropection: false,
						});
					}
					OperationDefinition::SelectionSet(sel) => {
						return worker.traverse_selection(&Value::Null, &sel, TraverserInfo {
							class_name: "Query",
							instropection: false,
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

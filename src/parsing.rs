use graphql_parser::query::*;
use serde_json::Value;

use super::indexing;
use super::schema;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs::File;
use std::io::Read;

struct QueryParser {
	pub schema: schema::SchemaClasses,
	pub database: Value,
	pub hashmaps: indexing::DatabaseHashmaps,
}

#[derive(Debug)]
struct ResolverInfo<'a, 'b> {
	field_name: &'a str,
	field_type: &'b schema::SchemaFieldReturnType,
	fragments: &'a HashMap<String, FragmentDefinition>,
}

#[derive(Debug, Clone)]
struct TraverserInfo<'a> {
	class_name: &'a str,
	fragments: &'a HashMap<String, FragmentDefinition>,
}

fn database() -> Value {
	let uri = String::from("public/data.json");
	let mut file = File::open(uri).expect("Unable to open");
	let mut data = String::new();
	file.read_to_string(&mut data).expect("Empty");
	serde_json::from_str(&data).unwrap()
}

fn gql2serde_value(v: &graphql_parser::query::Value) -> Value {
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
	fn find_match(
		&self,
		class_name: &String,
		iter: &Vec<Value>,
		args: &Vec<(String, graphql_parser::query::Value)>,
	) -> Value {
		let mut result: Vec<Value> = iter
			.iter()
			.map(|x| match &self.database[&class_name] {
				Value::Array(arr) => arr.iter().find(|y| &y["id"] == x).unwrap().clone(),
				_ => Value::Null.clone(),
			})
			.collect();
		for (key, val) in args {
			let val2 = gql2serde_value(val);
			result = result
				.into_iter()
				.filter(|x| x[&key] == Value::Null || x[&key] == val2)
				.collect();
		}

		json!(result)
	}

	// Resolve/Expand JSON database to object representation (by looking their Schema Type)
	fn resolve_id_to_object(&self, id: &Value, class_name: &String) -> Value {
		match id {
			Value::Array(arr) => {
				// Unpack array and resolve individually

				json!(arr
					.iter()
					.map(|x| self.resolve_id_to_object(x, &class_name))
					.collect::<Vec<Value>>())
			}
			x => {
				match &class_name[..] {
					// A primitive
					"String" | "ID" | "Number" | "Float" => id.clone(),
					// Object in schema
					n @ _ => match &self.database[&n] {
						Value::Array(arr) => {
							// Unpack object
							let idkey = match &self.hashmaps[&n[..]] {
								Some(v) => &v["id"],
								_ => {
									panic!();
								}
							};

							arr[match &idkey {
								indexing::FieldHashmaps::String(h) => h[&x.as_str().unwrap()[..]],
								indexing::FieldHashmaps::I32(h) => {
									h[&x.as_i64().unwrap().try_into().unwrap()]
								}
								indexing::FieldHashmaps::U64(h) => h[&x.as_u64().unwrap()],
								_ => panic!(),
							}]
							.clone()
						}
						_ => id.clone(),
					},
				}
			}
		}
	}

	pub fn resolve_field(
		&self,
		parent: &Value,
		args: &Vec<(String, graphql_parser::query::Value)>,
		context: &Field,
		info: ResolverInfo,
	) -> Value {
		if parent[&info.field_name] == Value::Null {
			return Value::Null;
		}
		let par = match &parent[&info.field_name] {
			Value::Array(arr) => {
				if !info.field_type.is_array {
					self.resolve_id_to_object(&arr[0], &info.field_type.name_type)
				} else {
					self.find_match(&info.field_type.name_type, arr, args)
				}
			}
			n @ _ => self.resolve_id_to_object(&n, &info.field_type.name_type),
		};

		self.traverse_selection(
			&par,
			&context.selection_set,
			TraverserInfo {
				class_name: &info.field_type.name_type[..],
				fragments: info.fragments,
			},
		)
	}

	pub fn traverse_selection(
		&self,
		parent: &Value,
		context: &SelectionSet,
		info: TraverserInfo,
	) -> Value {
		match parent {
			Value::Array(arr) => {
				let mut values: Vec<Value> = Vec::new();

				for obj in arr {
					values.push(self.traverse_selection(&obj, &context, info.clone()));
				}

				json!(values)
			}
			_ => {
				match &info.class_name[..] {
					"String" | "ID" | "Number" | "Float" => parent.clone(),
					_ => {
						let sch = match self.schema.get(&info.class_name[..]) {
							Some(v) => v,
							_ => {
								return Value::Null;
							}
						};
						match &sch {
							schema::SchemaType::Enum(_) => parent.clone(),
							_ => {
								let mut values: HashMap<Name, Value> = HashMap::new();
								for sel in &context.items {
									match &sel {
										// Field for parent
										Selection::Field(field) => match &sch {
											schema::SchemaType::Object(fields) => {
												values.insert(
													field.name.clone(),
													match &fields.contains_key(&field.name) {
														true => self.resolve_field(
															&parent,
															&field.arguments,
															&field,
															ResolverInfo {
																field_name: &field.name[..],
																field_type: &fields[&field.name]
																	.return_type,
																fragments: &info.fragments,
															},
														),
														false => Value::Null,
													},
												);
											}
											schema::SchemaType::Enum(_) => {
												values.insert(field.name.clone(), parent.clone());
											}
										},
										Selection::FragmentSpread(spread) => {
											// traverse again, then unpack
											let frag_values = self.traverse_selection(
												parent,
												&info.fragments[&spread.fragment_name]
													.selection_set,
												info.clone(),
											);
											match frag_values {
												Value::Object(obj) => {
													for (name, val) in obj {
														values.insert(name, val);
													}
												}
												_ => panic!(),
											}
										}
										_ => {}
									};
								}
								json!(values)
							}
						}
					}
				}
			}
		}
	}
}

pub fn traverse_query(ast: &Document) -> Value {
	// Load necessary files (should be done before server starts, actually)
	let mut sch = schema::traverse_schema("schema.gql");
	let instropection = schema::build_schema_instropection();
	let mut db = database();
	let dbmut = match db.as_object_mut() {
		Some(o) => o,
		_ => panic!("Database must be object!"),
	};
	dbmut.extend(instropection.database.as_object().unwrap().clone());
	dbmut["Query"][0]["id"] = json!("Query");
	dbmut["Query"][0]["__schema"] = json!("__Schema");
	db = json!(dbmut);
	let mut qhash = match &sch["Query"] {
		schema::SchemaType::Object(o) => o.clone(),
		_ => panic!(),
	};
	qhash.insert(
		"id".to_owned(),
		schema::SchemaField {
			name: "id".to_owned(),
			description: "".to_owned(),
			data_type: schema::SchemaFieldDataType {
				name_type: "string".to_owned(),
				is_array: false,
				is_unique: false,
				is_indexed: true,
			},
			return_type: schema::SchemaFieldReturnType {
				is_array: false,
				is_array_non_nullable: false,
				name_type: "ID".to_owned(),
				is_type_non_nullable: true,
			},
		},
	);
	qhash.insert(
		"__schema".to_owned(),
		schema::SchemaField {
			name: "__schema".to_owned(),
			description: "".to_owned(),
			data_type: schema::SchemaFieldDataType {
				name_type: "__Schema".to_owned(),
				is_array: false,
				is_unique: false,
				is_indexed: false,
			},
			return_type: schema::SchemaFieldReturnType {
				is_array: false,
				is_array_non_nullable: false,
				name_type: "__Schema".to_owned(),
				is_type_non_nullable: true,
			},
		},
	);
	sch.remove("Query");
	sch.insert("Query".to_owned(), schema::SchemaType::Object(qhash));
	sch.extend(instropection.schema);

	let hashmap = indexing::build_hashmaps(&db, &sch);
	let worker = QueryParser {
		schema: sch,
		database: db,
		hashmaps: hashmap,
	};
	let mut fragments = HashMap::new();
	// Look for fragments before doing actual operation
	for def in &ast.definitions {
		match &def {
			Definition::Fragment(fragdef) => {
				fragments.insert(fragdef.name.clone(), fragdef.clone());
			}
			_ => {}
		}
	}
	// Start action
	for def in &ast.definitions {
		match &def {
			Definition::Operation(opdef) => match &opdef {
				OperationDefinition::Query(query) => {
					return worker.traverse_selection(
						&worker.database["Query"][0],
						&query.selection_set,
						TraverserInfo {
							class_name: "Query",
							fragments: &fragments,
						},
					);
				}
				OperationDefinition::SelectionSet(sel) => {
					return worker.traverse_selection(
						&worker.database["Query"][0],
						&sel,
						TraverserInfo {
							class_name: "Query",
							fragments: &fragments,
						},
					);
				}

				_ => return json!("Other op"),
			},
			_ => return json!("Other def"),
		}
	}
	return json!("");
}

use graphql_parser::query::*;
use serde_json::Value;

use super::{indexing, schema, utility};
use std::collections::HashMap;

pub type DatabaseIndex = HashMap<String, Vec<Value>>;

#[derive(Clone)]
pub struct QueryParser {
	pub schema: schema::SchemaClasses,
	pub database: DatabaseIndex,
	pub hashmaps: indexing::DatabaseHashmaps,
}

#[derive(Debug)]
struct ResolverInfo<'a, 'b> {
	field_name: &'a str,
	field_type: &'b schema::SchemaFieldReturnType,
	fragments: &'a HashMap<String, &'a FragmentDefinition>,
	variables: &'a serde_json::Map<String, Value>,
}

#[derive(Debug, Clone)]
struct TraverserInfo<'a> {
	class_name: &'a str,
	fragments: &'a HashMap<String, &'a FragmentDefinition>,
	variables: &'a serde_json::Map<String, Value>,
}

fn read_database(db: &str) -> DatabaseIndex {
	let data = utility::read_db_file(&format!("{}/data.json", db)[..]);
	serde_json::from_str(&data).expect("File `database/data.json` is not valid JSON object!")
}

impl QueryParser {
	pub fn new(dbpath: &str) -> QueryParser {
		// Load necessary files (should be done before server starts, actually)
		let mut sch = schema::traverse_schema(&format!("{}/schema.gql", dbpath)[..]);
		let instropection = schema::build_schema_instropection(dbpath);
		let mut db = read_database(dbpath);
		db.extend(instropection.database);
		// Query pre-index injects
		{
			let qdata = db.get_mut("Query").unwrap()[0].as_object_mut().unwrap();
			let qhash = match sch.get_mut("Query").unwrap() {
				schema::SchemaType::Object(o) => o,
				_ => panic!(),
			};
			qdata.insert("id".to_owned(), json!("Query"));
			qdata.insert("__schema".to_owned(), json!("__Schema"));

			qhash.insert("id".to_owned(), schema::SchemaField::new("id", "ID", false));
			qhash.insert(
				"__schema".to_owned(),
				schema::SchemaField::new("__schema", "__Schema", false),
			);
		}
		// Mutation pre-index injects
		match sch.get_mut("Mutation") {
			Some(mut_mut) => {
				db.insert(
					"Mutation".to_owned(),
					vec![json!({"id": "Mutation".to_owned()})],
				);
				let qhash = match mut_mut {
					schema::SchemaType::Object(o) => o,
					_ => panic!(),
				};
				qhash.insert("id".to_owned(), schema::SchemaField::new("id", "ID", false));
			}, _ => {},
		};
		let mut qdata = HashMap::new();
		let mut qhash = HashMap::new();

		for (key, v) in &sch {
			match v {
				schema::SchemaType::Enum(_) => continue,
				_ => {}
			};

			let arr = &db
				.get(key)
				.expect(&format!("`{}` class is not found on schema", key)[..]);
			match &sch[key] {
				schema::SchemaType::Object(o) => {
					let hashes = match &o["id"].data_type.name_type[..] {
						"string" => json!(indexing::subindex_keys(arr, |value| {
							value["id"].as_str().unwrap().to_string()
						})),
						"i32" => json!(indexing::subindex_keys(arr, |value| {
							value["id"].as_i64().unwrap() as i32
						})),
						"u64" => json!(indexing::subindex_keys(arr, |value| {
							value["id"].as_u64().unwrap()
						})),
						_ => panic!("id is not indexable!"),
					};
					let fmtr = format!("values__of_{}", key);
					qdata.insert(fmtr.clone(), hashes);
					qhash.insert(fmtr.clone(), schema::SchemaField::new(&fmtr, key, true));
				}
				_ => {}
			}
		}
		// Des tak des
		db.get_mut("Query").unwrap()[0]
			.as_object_mut()
			.unwrap()
			.extend(qdata);
		match sch.get_mut("Query").unwrap() {
			schema::SchemaType::Object(o) => o.extend(qhash),
			_ => panic!(),
		};
		sch.extend(instropection.schema);
		let hashmap = indexing::build_hashmaps(&db, &sch);
		QueryParser {
			schema: sch,
			database: db,
			hashmaps: hashmap,
		}
	}

	// Given Values, Apply filters to it
	fn _find_match(
		&self,
		class_name: &str,
		iter: &Vec<Value>,
		args: &Vec<(String, graphql_parser::query::Value)>,
	) -> Value {
		self.database.get(class_name).expect(&format!("{} not found", class_name)[..]);
		let mut result: Vec<Value> = iter
			.iter()
			.map(|x| {
				self.database[class_name]
					.iter()
					.find(|y| &y["id"] == x)
					.unwrap_or(&Value::Null)
					.clone()
			})
			.collect();
		for (key, val) in args {
			let val2 = utility::gql2serde_value(val);
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
					"String" | "ID" | "Number" | "Float" | "Int" => id.clone(),
					// Object in schema
					n @ _ => {
						let arr = match self.database.get(&n[..]) {
							Some(v) => v,
							_ => {
								return id.clone();
							} // Could be an enum
						};
						// Unpack object
						let idkey = match &self.hashmaps[n] {
							Some(v) => &v["id"],
							_ => {
								panic!();
							}
						};

						let keyy = match &idkey {
							indexing::FieldHashmaps::String(h) => match x.as_str() {
								Some(v) => match h.get(v) {
									Some(v) => Some(v),
									_ => None,
								},
								_ => None,
							},
							indexing::FieldHashmaps::I32(h) => {
								Some(&h[&(x.as_i64().unwrap() as i32)])
							}
							indexing::FieldHashmaps::U64(h) => Some(&h[&(x.as_u64().unwrap())]),
							_ => panic!(),
						};
						match keyy {
							Some(v) => arr[*v].clone(),
							_ => Value::Null,
						}
					}
				}
			}
		}
	}

	fn resolve_field(
		&self,
		parent: &Value,
		_args: &Vec<(String, graphql_parser::query::Value)>,
		context: &Field,
		info: ResolverInfo,
	) -> Value {
		let par = match &parent[&info.field_name] {
			Value::Array(arr) => {
				if !info.field_type.is_array {
					self.resolve_id_to_object(&arr[0], &info.field_type.name_type)
				} else {
					self.resolve_id_to_object(&parent[&info.field_name], &info.field_type.name_type)
					//self.find_match(&info.field_type.name_type, arr, args)
				}
			}
			Value::Null => return Value::Null, // resolving null is null
			n @ _ => self.resolve_id_to_object(&n, &info.field_type.name_type),
		};

		self.traverse_selection(
			&par,
			&context.selection_set,
			TraverserInfo {
				class_name: &info.field_type.name_type[..],
				fragments: info.fragments,
				variables: info.variables,
			},
		)
	}

	fn traverse_selection(
		&self,
		parent: &Value,
		context: &SelectionSet,
		info: TraverserInfo,
	) -> Value {
		match parent {
			Value::Array(arr) => json!(arr
				.iter()
				.map(|obj| self.traverse_selection(&obj, &context, info.clone()))
				.collect::<Vec<Value>>()),
			_ => {
				match &info.class_name[..] {
					"String" | "ID" | "Number" | "Float" | "Int" => parent.clone(),
					nn @ _ => {
						match &self.schema.get(nn) {
							Some(schema::SchemaType::Enum(_)) => parent.clone(),
							Some(schema::SchemaType::Object(fields)) => {
								let mut values = HashMap::new();
								for sel in &context.items {
									match sel {
										// Field for parent
										Selection::Field(field) => {
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
															variables: &info.variables,
														},
													),
													false => Value::Null,
												},
											);
										}
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
											};
										}
										_ => {}
									};
								}
								json!(values)
							}
							_ => Value::Null,
						}
					}
				}
			}
		}
	}

	pub fn traverse_query(
		&self,
		ast: &Document,
		variables: &serde_json::Map<String, Value>,
	) -> Value {
		// Look for fragments before doing actual operation
		let fragments = ast
			.definitions
			.iter()
			.filter_map(|def| match &def {
				Definition::Fragment(fragdef) => Some((fragdef.name.clone(), fragdef)),
				_ => None,
			})
			.collect::<HashMap<String, &FragmentDefinition>>();

		// Start action
		let subset = ast
			.definitions
			.iter()
			.filter_map(|def| match &def {
				Definition::Operation(opdef) => match opdef {
					OperationDefinition::Query(q) => Some((&q.selection_set, "Query")),
					OperationDefinition::SelectionSet(s) => Some((s, "Query")),
					OperationDefinition::Mutation(m) => Some((&m.selection_set, "Mutation")),
					OperationDefinition::Subscription(s) => {
						Some((&s.selection_set, "Subscription"))
					}
				},
				_ => None,
			})
			.collect::<Vec<(&SelectionSet, &str)>>()[0];

		self.traverse_selection(
			&self.database[subset.1][0],
			&subset.0,
			TraverserInfo {
				class_name: subset.1,
				fragments: &fragments,
				variables: &variables,
			},
		)
	}
}

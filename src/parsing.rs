use graphql_parser::query::*;
use serde_json::Value as JSONValue;

use super::{indexing, schema, structure, utility};
use std::collections::HashMap;

pub type DatabaseIndex = HashMap<String, Vec<JSONValue>>;

#[derive(Clone)]
pub struct QueryParser {
	pub schema: structure::StructureIndex,
	pub database: DatabaseIndex,
	pub hashmaps: indexing::DatabaseHashmaps,
}

#[derive(Debug)]
struct ResolverInfo<'a, 'b> {
	field_name: &'a str,
	field_type: &'b structure::StructureReturnType,
	fragments: &'a HashMap<String, &'a FragmentDefinition>,
	variables: &'a serde_json::Map<String, JSONValue>,
}

#[derive(Debug, Clone)]
struct TraverserInfo<'a> {
	class_name: &'a str,
	fragments: &'a HashMap<String, &'a FragmentDefinition>,
	variables: &'a serde_json::Map<String, JSONValue>,
}

pub fn read_database(db: &str) -> DatabaseIndex {
	let data = utility::read_db_file(&format!("{}/data.json", db)[..]);
	serde_json::from_str(&data).expect("File `database/data.json` is not valid JSON object!")
}

impl QueryParser {
	pub fn new(database: DatabaseIndex, schema: structure::StructureIndex) -> QueryParser {
		// Load necessary files (should be done before server starts, actually)
		let mut schema = schema;
		let instropection = schema::build_schema_instropection(&schema);
		let mut db = database;
		db.extend(instropection.database);
		// Query pre-index injects
		{

			let qdata = db.get_mut("Query").unwrap()[0].as_object_mut().unwrap();
			let qhash = match schema.find_object_mut("Query") {
				structure::StructureItemMut::Object(o) => o,
				_ => panic!(),
			};
			qdata.insert("id".to_owned(), json!("Query"));
			qdata.insert("__schema".to_owned(), json!("__Schema"));
			qhash.add_field(structure::StructureField::from(
				"id".to_owned(),
				"".to_owned(),
				"ID".to_owned(),
				false,
			));
			qhash.add_field(structure::StructureField::from(
				"__schema".to_owned(),
				"".to_owned(),
				"__Schema".to_owned(),
				false,
			));
		}
		{

			// Mutation pre-index injects
			match schema.find_object_mut("Mutation") {
				structure::StructureItemMut::Object(qhash) => {
					db.insert(
						"Mutation".to_owned(),
						vec![json!({"id": "Mutation".to_owned()})],
					);
					qhash.add_field(structure::StructureField::from(
						"id".to_owned(),
						"".to_owned(),
						"ID".to_owned(),
						false,
					));
				}
				_ => {}
			};
		}
		let mut qdata = HashMap::new();
		let mut qhash = Vec::new();

		for v in &schema.objects {
			let arr = &db
				.get(&v.name)
				.expect(&format!("`{}` class is not found on schema", v.name)[..]);
			match &schema.find_object(&v.name) {
				structure::StructureItem::Object(o) => {
					let hashes = match &o.find_field("id").expect("No ID!").data_type.kind[..] {
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
					let fmtr = format!("values__of_{}", v.name);
					qdata.insert(fmtr.clone(), hashes);
					qhash.push(structure::StructureField::from(
						fmtr.clone(),
						"".to_owned(),
						v.name.clone(),
						true,
					));
				}
				_ => {}
			}
		}
		// Des tak des
		db.get_mut("Query").unwrap()[0]
			.as_object_mut()
			.unwrap()
			.extend(qdata);
		match schema.find_object_mut("Query") {
			structure::StructureItemMut::Object(o) => {
				for qqq in qhash {
					o.add_field(qqq);
				}
			}
			_ => panic!(),
		};
		for ooo in instropection.schema.objects {
			schema.add_object(ooo);
		}
		for ooo in instropection.schema.enums {
			schema.add_enum(ooo);
		}
		let hashmap = indexing::build_hashmaps(&db, &schema);
		QueryParser {
			schema: schema,
			database: db,
			hashmaps: hashmap,
		}
	}

	// Given Values, Apply filters to it
	fn _find_match(
		&self,
		class_name: &str,
		iter: &Vec<JSONValue>,
		args: &Vec<(String, graphql_parser::query::Value)>,
	) -> JSONValue {
		self.database
			.get(class_name)
			.expect(&format!("{} not found", class_name)[..]);
		let mut result: Vec<JSONValue> = iter
			.iter()
			.map(|x| {
				self.database[class_name]
					.iter()
					.find(|y| &y["id"] == x)
					.unwrap_or(&JSONValue::Null)
					.clone()
			})
			.collect();
		for (key, val) in args {
			let val2 = utility::gql2serde_value(val);
			result = result
				.into_iter()
				.filter(|x| x[&key] == JSONValue::Null || x[&key] == val2)
				.collect();
		}

		json!(result)
	}

	// Resolve/Expand JSON database to object representation (by looking their Schema Type)
	fn resolve_id_to_object(&self, id: &JSONValue, class_name: &String) -> JSONValue {
		match id {
			JSONValue::Array(arr) => {
				// Unpack array and resolve individually

				json!(arr
					.iter()
					.map(|x| self.resolve_id_to_object(x, &class_name))
					.collect::<Vec<JSONValue>>())
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
							Some(v) => arr[v[0]].clone(),
							_ => JSONValue::Null,
						}
					}
				}
			}
		}
	}

	fn resolve_field(
		&self,
		parent: &JSONValue,
		_args: &Vec<(String, graphql_parser::query::Value)>,
		context: &Field,
		info: ResolverInfo,
	) -> JSONValue {
		let par = match &parent[&info.field_name] {
			JSONValue::Array(arr) if !info.field_type.is_array => {
				self.resolve_id_to_object(&arr[0], &info.field_type.name)
			}
			JSONValue::Null => return JSONValue::Null, // resolving null is null
			n @ _ => self.resolve_id_to_object(&n, &info.field_type.name),
		};

		self.traverse_selection(
			&par,
			&context.selection_set,
			TraverserInfo {
				class_name: &info.field_type.name[..],
				fragments: info.fragments,
				variables: info.variables,
			},
		)
	}

	fn traverse_selection(
		&self,
		parent: &JSONValue,
		context: &SelectionSet,
		info: TraverserInfo,
	) -> JSONValue {
		match parent {
			JSONValue::Array(arr) => json!(arr
				.iter()
				.map(|obj| self.traverse_selection(&obj, &context, info.clone()))
				.collect::<Vec<JSONValue>>()),
			_ => {
				match &info.class_name[..] {
					"String" | "ID" | "Number" | "Float" | "Int" => parent.clone(),
					nn @ _ => {
						match &self.schema.find_object(nn) {
							structure::StructureItem::Enum(_) => parent.clone(),
							structure::StructureItem::Object(fields) => {
								let mut values = HashMap::new();
								for sel in &context.items {
									match sel {
										// Field for parent
										Selection::Field(field) => {
											values.insert(
												field.name.clone(),
												match &fields.find_field(&field.name) {
													Some(ff) => self.resolve_field(
														&parent,
														&field.arguments,
														&field,
														ResolverInfo {
															field_name: &field.name[..],
															field_type: &ff.return_type,
															fragments: &info.fragments,
															variables: &info.variables,
														},
													),
													None => JSONValue::Null,
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
												JSONValue::Object(obj) => {
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
							_ => JSONValue::Null,
						}
					}
				}
			}
		}
	}

	pub fn traverse_query(
		&self,
		ast: &Document,
		variables: &serde_json::Map<String, JSONValue>,
	) -> JSONValue {
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

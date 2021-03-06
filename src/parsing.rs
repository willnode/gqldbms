use graphql_parser::query::*;
use serde_json::Value as JSONValue;

use super::{indexing, resolver, schema, structure};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub type DatabaseIndex = HashMap<String, Vec<JSONValue>>;

pub type DatabaseDirectory = Arc<RwLock<HashMap<String, QueryParser>>>;

#[derive(Clone)]
pub struct QueryParser {
	pub schema: structure::StructureIndex,
	pub database: DatabaseIndex,
	pub hashmaps: indexing::DatabaseHashmaps,
	pub is_canonical: bool,
	pub directory: DatabaseDirectory,
}

impl QueryParser {
	pub fn new(
		database: DatabaseIndex,
		schema: structure::StructureIndex,
		instropection: structure::StructureIndex,
		directory: DatabaseDirectory
	) -> QueryParser {
		// Load necessary files (should be done before server starts, actually)
		let mut schema = schema;
		let instropection = schema::build_schema_instropection(&schema, instropection);
		let mut db = database;
		db.extend(instropection.database);
		// Query schema injects
		{
			let qhash = match schema.find_object_mut("Query") {
				structure::StructureItemMut::Object(o) => o,
				_ => panic!(),
			};
			qhash.add_field(structure::StructureField::from(
				"__schema".to_owned(),
				"".to_owned(),
				"__Schema".to_owned(),
				false,
				Some(structure::StructureDataResolver {
					args: Vec::default(),
					flags: Vec::default(),
					kind: "ALL_REFERENCES".to_owned(),
				}),
			));
		}
		// let mut qdata = HashMap::new();
		let mut qhash = Vec::new();

		for v in &schema.objects {
			match &schema.find_object(&v.name) {
				structure::StructureItem::Object(_) => {
					let fmtr = format!("values__of_{}", v.name);
					// qdata.insert(fmtr.clone(), hashes);
					qhash.push(structure::StructureField::from(
						fmtr.clone(),
						"".to_owned(),
						v.name.clone(),
						true,
						Some(structure::StructureDataResolver {
							args: Vec::default(),
							flags: Vec::default(),
							kind: "ALL_REFERENCES".to_owned(),
						}),
					));
				}
				_ => {}
			}
		}
		// we require all of these has in DB, altough has no members at all
		for class in vec!["Query", "Mutation", "Subscription"] {
			if !db.contains_key(class) {
				db.insert(class.to_owned(), vec![json!({})]);
			}
		}

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
		// println!("{}", json!(db));
		let hashmap = indexing::build_hashmaps(&db, &schema);
		let is_canonical = schema.name == "canonical";
		QueryParser {
			schema: schema,
			database: db,
			hashmaps: hashmap,
			is_canonical: is_canonical,
			directory: directory,
		}
	}

	// Resolve/Expand JSON database to object representation (by looking their Schema Type)
	fn resolve_id_to_object(&self, id: &JSONValue, class_name: &String) -> JSONValue {
		match id {
			// Unpack array and resolve individually
			JSONValue::Array(arr) => json!(arr
				.iter()
				.filter_map(|x| match x {
					JSONValue::Null => None,
					_ => match self.resolve_id_to_object(x, &class_name) {
						JSONValue::Null => None,
						y @ _ => Some(y),
					},
				})
				.collect::<Vec<JSONValue>>()),
			// Null is null
			JSONValue::Null => JSONValue::Null,
			x => {
				match class_name.as_ref() {
					// A primitive
					"String" | "ID" | "Number" | "Float" | "Int" => id.clone(),
					// Object in schema
					n @ _ => {
						let arr = match self.database.get(n) {
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
		args: &Vec<(String, graphql_parser::query::Value)>,
		selector: &graphql_parser::query::Field,
		context: &resolver::GenericResolverContext,
		info: &structure::StructureField,
	) -> JSONValue {
		println!("I am dead");
		let written = &mut (*self.directory.write().unwrap());
		println!("I am alive");
		match resolver::resolve(parent, args, &resolver::ResolverContext {
			fragments: context.fragments,
			variables: context.variables,
			parser: written.get_mut(&self.schema.name[..]).unwrap(),
		}, info) {
			JSONValue::Null => JSONValue::Null,
			results @ _ => self.traverse_selection(
				&self.resolve_id_to_object(&results, &info.return_type.name),
				&selector.selection_set,
				context,
				&info.return_type.name,
			),
		}
	}

	fn traverse_selection(
		&self,
		parent: &JSONValue,
		selector: &SelectionSet,
		context: &resolver::GenericResolverContext,
		info: &str,
	) -> JSONValue {
		match parent {
			JSONValue::Array(arr) => json!(arr
				.iter()
				.map(|obj| self.traverse_selection(&obj, selector, context, info))
				.collect::<Vec<JSONValue>>()),
			_ => {
				match info {
					"String" | "ID" | "Number" | "Float" | "Int" => parent.clone(),
					nn @ _ => {
						match &self.schema.find_object(nn) {
							structure::StructureItem::Enum(_) => parent.clone(),
							structure::StructureItem::Object(fields) => {
								let mut values = HashMap::new();
								for sel in &selector.items {
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
														context,
														&ff,
													),
													None => JSONValue::Null,
												},
											);
										}
										Selection::FragmentSpread(spread) => {
											// traverse again, then unpack
											let frag_values = self.traverse_selection(
												parent,
												&context.fragments[&spread.fragment_name]
													.selection_set,
												context,
												info,
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
			&resolver::GenericResolverContext {
				fragments: &fragments,
				variables: &variables,
			},
			subset.1,
		)
	}
}

use std::sync::MutexGuard;
use atomic_lib::{Resource, storelike::Property};
use serde::Serialize;
use crate::{appstate::AppState, render::propvals::HTMLAtom};

#[derive(Serialize)]
struct CollectionTable {
  header: Vec<Property>,
  members: Vec<Vec<HTMLAtom>>,
}

pub fn render_collection(resource: &Resource, context: &MutexGuard<AppState>) -> String {
  let json = resource.to_json(&context.store, 1, false).unwrap();
  let body  = format!("{}", json);
  let header: Vec<Property>;
  body
}
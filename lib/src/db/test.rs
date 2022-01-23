use crate::urls;

use super::*;
use ntest::timeout;

/// Creates new temporary database, populates it, removes previous one.
/// Can only be run one thread at a time, because it requires a lock on the DB file.
fn init(id: &str) -> Db {
    let tmp_dir_path = format!(".temp/db/{}", id);
    let _try_remove_existing = std::fs::remove_dir_all(&tmp_dir_path);
    let store = Db::init(
        std::path::Path::new(&tmp_dir_path),
        "https://localhost".into(),
    )
    .unwrap();
    let agent = store.create_agent(None).unwrap();
    store.set_default_agent(agent);
    store.populate().unwrap();
    store
}

/// Share the Db instance between tests. Otherwise, all tests try to init the same location on disk and throw errors.
/// Note that not all behavior can be properly tested with a shared database.
/// If you need a clean one, juts call init("someId").
use lazy_static::lazy_static; // 1.4.0
use std::sync::Mutex;
lazy_static! {
    pub static ref DB: Mutex<Db> = Mutex::new(init("shared"));
}

#[test]
#[timeout(30000)]
fn basic() {
    let store = DB.lock().unwrap().clone();
    // We can create a new Resource, linked to the store.
    // Note that since this store only exists in memory, it's data cannot be accessed from the internet.
    // Let's make a new Property instance!
    let mut new_resource =
        crate::Resource::new_instance("https://atomicdata.dev/classes/Property", &store).unwrap();
    // And add a description for that Property
    new_resource
        .set_propval_shortname("description", "the age of a person", &store)
        .unwrap();
    new_resource
        .set_propval_shortname("shortname", "age", &store)
        .unwrap();
    new_resource
        .set_propval_shortname("datatype", crate::urls::INTEGER, &store)
        .unwrap();
    // Changes are only applied to the store after saving them explicitly.
    new_resource.save_locally(&store).unwrap();
    // The modified resource is saved to the store after this

    // A subject URL has been created automatically.
    let subject = new_resource.get_subject();
    let fetched_new_resource = store.get_resource(subject).unwrap();
    let description_val = fetched_new_resource
        .get_shortname("description", &store)
        .unwrap()
        .to_string();
    assert!(description_val == "the age of a person");

    // Try removing something
    store.get_resource(crate::urls::CLASS).unwrap();
    store.remove_resource(crate::urls::CLASS).unwrap();
    // Should throw an error, because can't remove non-existent resource
    store.remove_resource(crate::urls::CLASS).unwrap_err();
    // Should throw an error, because resource is deleted
    store.get_propvals(crate::urls::CLASS).unwrap_err();

    assert!(store.all_resources(false).len() < store.all_resources(true).len());
}

#[test]
fn populate_collections() {
    let store = DB.lock().unwrap().clone();
    let subjects: Vec<String> = store
        .all_resources(false)
        .into_iter()
        .map(|r| r.get_subject().into())
        .collect();
    println!("{:?}", subjects);
    let collections_collection_url = format!("{}/collections", store.get_server_url());
    let collections_resource = store
        .get_resource_extended(&collections_collection_url, false, None)
        .unwrap();
    let member_count = collections_resource
        .get(crate::urls::COLLECTION_MEMBER_COUNT)
        .unwrap()
        .to_int()
        .unwrap();
    assert!(member_count > 11);
    let nested = collections_resource
        .get(crate::urls::COLLECTION_INCLUDE_NESTED)
        .unwrap()
        .to_bool()
        .unwrap();
    assert!(nested);
}

#[test]
/// Check if the cache is working
fn add_atom_to_index() {
    let store = DB.lock().unwrap().clone();
    let subject = urls::CLASS.into();
    let property: String = urls::PARENT.into();
    let val_string = urls::AGENT;
    let value = Value::new(val_string, &crate::datatype::DataType::AtomicUrl).unwrap();
    // This atom should normally not exist - Agent is not the parent of Class.
    let atom = Atom::new(subject, property.clone(), value);
    store
        .add_atom_to_index(&atom, &Resource::new("ds".into()))
        .unwrap();
    let found_no_external = store
        .tpf(None, Some(&property), Some(val_string), false)
        .unwrap();
    // Don't find the atom if no_external is true.
    assert_eq!(
        found_no_external.len(),
        0,
        "found items - should ignore external items"
    );
    let found_external = store
        .tpf(None, Some(&property), Some(val_string), true)
        .unwrap();
    // If we see the atom, it's in the index.
    assert_eq!(found_external.len(), 1);
}

#[test]
/// Check if a resource is properly removed from the DB after a delete command.
/// Also counts commits.
fn destroy_resource_and_check_collection_and_commits() {
    let store = init("counter");
    let agents_url = format!("{}/agents", store.get_server_url());
    let agents_collection_1 = store
        .get_resource_extended(&agents_url, false, None)
        .unwrap();
    let agents_collection_count_1 = agents_collection_1
        .get(crate::urls::COLLECTION_MEMBER_COUNT)
        .unwrap()
        .to_int()
        .unwrap();
    assert_eq!(
        agents_collection_count_1, 1,
        "The Agents collection is not one (we assume there is one agent already present from init)"
    );

    // We will count the commits, and check if they've incremented later on.
    let commits_url = format!("{}/commits", store.get_server_url());
    let commits_collection_1 = store
        .get_resource_extended(&commits_url, false, None)
        .unwrap();
    let commits_collection_count_1 = commits_collection_1
        .get(crate::urls::COLLECTION_MEMBER_COUNT)
        .unwrap()
        .to_int()
        .unwrap();
    println!("Commits collection count 1: {}", commits_collection_count_1);

    let mut resource = crate::agents::Agent::new(None, &store)
        .unwrap()
        .to_resource(&store)
        .unwrap();
    resource.save_locally(&store).unwrap();
    let agents_collection_2 = store
        .get_resource_extended(&agents_url, false, None)
        .unwrap();
    let agents_collection_count_2 = agents_collection_2
        .get(crate::urls::COLLECTION_MEMBER_COUNT)
        .unwrap()
        .to_int()
        .unwrap();
    assert_eq!(
        agents_collection_count_2, 2,
        "The new Agent resource did not increase the collection member count."
    );

    let commits_collection_2 = store
        .get_resource_extended(&commits_url, false, None)
        .unwrap();
    let commits_collection_count_2 = commits_collection_2
        .get(crate::urls::COLLECTION_MEMBER_COUNT)
        .unwrap()
        .to_int()
        .unwrap();
    println!("Commits collection count 2: {}", commits_collection_count_2);
    assert_eq!(
        commits_collection_count_2,
        commits_collection_count_1 + 1,
        "The commits collection did not increase after saving the resource."
    );

    resource.destroy(&store).unwrap();
    let agents_collection_3 = store
        .get_resource_extended(&agents_url, false, None)
        .unwrap();
    let agents_collection_count_3 = agents_collection_3
        .get(crate::urls::COLLECTION_MEMBER_COUNT)
        .unwrap()
        .to_int()
        .unwrap();
    assert_eq!(
        agents_collection_count_3, 1,
        "The collection count did not decrease after destroying the resource."
    );

    let commits_collection_3 = store
        .get_resource_extended(&commits_url, false, None)
        .unwrap();
    let commits_collection_count_3 = commits_collection_3
        .get(crate::urls::COLLECTION_MEMBER_COUNT)
        .unwrap()
        .to_int()
        .unwrap();
    println!("Commits collection count 3: {}", commits_collection_count_3);
    assert_eq!(
        commits_collection_count_3,
        commits_collection_count_2 + 1,
        "The commits collection did not increase after destroying the resource."
    );
}

#[test]
fn get_extended_resource_pagination() {
    let store = DB.lock().unwrap().clone();
    let subject = format!("{}/commits?current_page=2", store.get_server_url());
    // Should throw, because page 2 is out of bounds for default page size
    let _wrong_resource = store
        .get_resource_extended(&subject, false, None)
        .unwrap_err();
    // let subject = "https://atomicdata.dev/classes?current_page=2&page_size=1";
    let subject_with_page_size = format!("{}&page_size=1", subject);
    let resource = store
        .get_resource_extended(&subject_with_page_size, false, None)
        .unwrap();
    let cur_page = resource
        .get(urls::COLLECTION_CURRENT_PAGE)
        .unwrap()
        .to_int()
        .unwrap();
    assert_eq!(cur_page, 2);
    assert_eq!(resource.get_subject(), &subject_with_page_size);
}

/// Generate a bunch of resources, query them.
/// Checks if cache is properly invalidated on modifying or deleting resources.
#[test]
fn query_cache_invalidation() {
    let store = &DB.lock().unwrap().clone();

    let demo_val = "myval".to_string();
    let demo_reference = urls::PARAGRAPH;

    let count = 10;
    let limit = 5;
    assert!(
        count > limit,
        "following tests might not make sense if count is less than limit"
    );

    let sort_by = urls::DESCRIPTION;

    for _x in 0..count {
        let mut demo_resource = Resource::new_generate_subject(store);
        // We make one resource public
        if _x == 1 {
            demo_resource
                .set_propval(urls::READ.into(), vec![urls::PUBLIC_AGENT].into(), store)
                .unwrap();
        }
        demo_resource
            .set_propval_string(urls::DESTINATION.into(), demo_reference, store)
            .unwrap();
        demo_resource
            .set_propval(urls::SHORTNAME.into(), Value::Slug(demo_val.clone()), store)
            .unwrap();
        demo_resource
            .set_propval(
                sort_by.into(),
                Value::Markdown(crate::utils::random_string()),
                store,
            )
            .unwrap();
        demo_resource.save(store).unwrap();
    }

    let mut q = Query {
        property: Some(urls::DESTINATION.into()),
        value: Some(demo_reference.into()),
        limit: Some(limit),
        start_val: None,
        end_val: None,
        offset: 0,
        sort_by: None,
        sort_desc: false,
        include_external: true,
        include_nested: false,
        for_agent: None,
    };
    let res = store.query(&q).unwrap();
    assert_eq!(
        res.count, count,
        "number of references without property filter"
    );
    assert_eq!(limit, res.subjects.len(), "limit");

    q.property = None;
    q.value = Some(demo_val);
    let res = store.query(&q).unwrap();
    assert_eq!(res.count, count, "literal value");

    q.offset = 9;
    let res = store.query(&q).unwrap();
    assert_eq!(res.subjects.len(), count - q.offset, "offset");
    assert_eq!(res.resources.len(), 0, "no nested resources");

    q.offset = 0;
    q.include_nested = true;
    let res = store.query(&q).unwrap();
    assert_eq!(res.resources.len(), limit, "nested resources");

    q.sort_by = Some(sort_by.into());
    let mut res = store.query(&q).unwrap();
    let mut prev_resource = res.resources[0].clone();
    // For one resource, we will change the order by changing its value
    let mut resource_changed_order_opt = None;
    for (i, r) in res.resources.iter_mut().enumerate() {
        let previous = prev_resource.get(sort_by).unwrap().to_string();
        let current = r.get(sort_by).unwrap().to_string();
        assert!(
            previous <= current,
            "should be ascending: {} - {}",
            previous,
            current
        );
        // We change the order!
        if i == 4 {
            r.set_propval(sort_by.into(), Value::Markdown("!first".into()), store)
                .unwrap();
            r.save(store).unwrap();
            resource_changed_order_opt = Some(r.clone());
        }
        prev_resource = r.clone();
    }

    let mut resource_changed_order = resource_changed_order_opt.unwrap();

    assert_eq!(res.count, count, "count changed after updating one value");

    q.sort_by = Some(sort_by.into());
    let res = store.query(&q).unwrap();
    assert_eq!(
        res.resources[0].get_subject(),
        resource_changed_order.get_subject(),
        "order did not change after updating resource"
    );

    resource_changed_order.destroy(store).unwrap();
    let res = store.query(&q).unwrap();
    assert!(
        res.resources[0].get_subject() != resource_changed_order.get_subject(),
        "deleted resoruce still in results"
    );

    q.sort_desc = true;
    let res = store.query(&q).unwrap();
    let first = res.resources[0].get(sort_by).unwrap().to_string();
    let later = res.resources[limit - 1].get(sort_by).unwrap().to_string();
    assert!(first > later, "sort by desc");

    q.for_agent = Some(urls::PUBLIC_AGENT.into());
    let res = store.query(&q).unwrap();
    assert_eq!(res.subjects.len(), 1, "authorized subjects");
    assert_eq!(res.resources.len(), 1, "authorized resources");
    // TODO: Ideally, the count is authorized too. But doing that could be hard. (or expensive)
    // https://github.com/joepio/atomic-data-rust/issues/286
    // assert_eq!(res.count, 1, "authorized count");
}

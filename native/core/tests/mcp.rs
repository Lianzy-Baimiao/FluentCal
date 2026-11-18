//! AI assistants (MCP, 需求文档 4.12 / 7.8): the protocol in both generations, every tool with
//! the editor's rules, the endpoint on a real loopback port, and the stdin/stdout bridge.
mod common;

use common::*;
use fluentcal_core::date::{add_days, start_of_month, today};
use fluentcal_core::mcp::proto::{self, Outcome};
use fluentcal_core::mcp::{bridge, http, tools, Change, Host};
use fluentcal_core::store::Store;
use fluentcal_core::tasks::Doc;
use serde_json::{json, Map, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const MODERN: &str = "2026-07-28";

/// The calendar of a test: a store, written straight away.
struct TestHost {
    store: Arc<Store>,
    write: AtomicBool,
}

impl Host for TestHost {
    fn doc(&self) -> Doc {
        Doc::from_value(&self.store.snapshot())
    }
    fn can_write(&self) -> bool {
        self.write.load(Ordering::Relaxed)
    }
    fn change(&self, f: Change) -> Result<Value, String> {
        let (edit, told) = f(&self.doc())?;
        edit.write(&self.store, "mcp")?;
        Ok(told)
    }
}

fn host(name: &str) -> Arc<TestHost> {
    Arc::new(TestHost { store: store(&format!("mcp-{name}")), write: AtomicBool::new(true) })
}

fn put(s: &Store, task: Value) {
    let mut m = Map::new();
    m.insert(task["id"].as_str().unwrap().to_owned(), task);
    s.put("tasks", &m);
}

fn put_group(s: &Store, id: &str, name: &str, order: i64) {
    let mut m = Map::new();
    m.insert(id.to_owned(), json!({ "id": id, "name": name, "color": "blue", "order": order }));
    s.put("groups", &m);
}

fn stored(h: &TestHost, id: &str) -> Value {
    h.store.snapshot()["tasks"][id].clone()
}

fn count(h: &TestHost) -> usize {
    h.store.snapshot()["tasks"].as_object().map_or(0, Map::len)
}

/// A request the stateless way: what it is said in `_meta`.
fn modern(method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({ (proto::META_VERSION): MODERN, (proto::META_CAPABILITIES): {}, "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "1" } });
    json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params })
}

/// A request the older way (after `initialize`).
fn legacy(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": "a-1", "method": method, "params": params })
}

fn reply(h: &dyn Host, msg: &Value) -> (Value, u16) {
    match proto::handle(h, msg) {
        Outcome::Reply { body, status } => (body, status),
        Outcome::Accepted => panic!("no answer to {msg}"),
    }
}

/// Calls a tool: Ok(what it told), Err(why not).
fn call(h: &dyn Host, tool: &str, args: Value) -> Result<Value, String> {
    let (body, status) = reply(h, &legacy("tools/call", json!({ "name": tool, "arguments": args })));
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["id"], "a-1");
    let r = &body["result"];
    assert_eq!(r["content"].as_array().map(Vec::len), Some(1), "{body}");
    assert_eq!(r["content"][0]["type"], "text");
    let text = r["content"][0]["text"].as_str().unwrap().to_owned();
    if r["isError"] == true {
        Err(text)
    } else {
        assert!(r.get("isError").is_none(), "{body}");
        Ok(serde_json::from_str(&text).unwrap_or_else(|_| panic!("not JSON: {text}")))
    }
}

fn err(h: &dyn Host, tool: &str, args: Value) -> String {
    call(h, tool, args.clone()).expect_err(&format!("{tool} {args} should not work"))
}

/// Adds one task; its id.
fn add(h: &dyn Host, task: Value) -> String {
    let told = call(h, "add_tasks", json!({ "tasks": [task] })).unwrap();
    assert_eq!(told["added"], 1);
    told["tasks"][0]["id"].as_str().unwrap().to_owned()
}

// ---------------------------------------------------------------------------------------
// The protocol
// ---------------------------------------------------------------------------------------

#[test]
fn the_stateless_protocol() {
    let h = host("modern");
    let (body, status) = reply(&*h, &modern("server/discover", json!({})));
    assert_eq!((status, &body["id"], &body["jsonrpc"]), (200, &json!(7), &json!("2.0")));
    let r = &body["result"];
    assert_eq!(r["resultType"], "complete");
    assert!(r["supportedVersions"].as_array().unwrap().contains(&json!(MODERN)));
    assert!(r["supportedVersions"].as_array().unwrap().contains(&json!("2025-11-25")), "dual-era: the older versions are named too");
    assert!(r["capabilities"]["tools"].is_object());
    assert_eq!(r["_meta"]["io.modelcontextprotocol/serverInfo"]["name"], "fluentcal");
    assert_eq!(r["_meta"]["io.modelcontextprotocol/serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(r["instructions"].as_str().unwrap().contains("YYYY-MM-DD"));
    assert!(r["ttlMs"].as_u64().is_some() && r["cacheScope"] == "public");

    let (body, status) = reply(&*h, &modern("tools/list", json!({})));
    assert_eq!(status, 200);
    let r = &body["result"];
    assert_eq!(r["resultType"], "complete");
    assert!(r["ttlMs"].as_u64().is_some() && r["cacheScope"] == "public" && r.get("nextCursor").is_none());
    let names: Vec<&str> = r["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["list_tasks", "search_tasks", "get_task", "list_groups", "list_todos", "get_days", "add_tasks", "update_task", "set_task_done", "delete_task", "add_group"]);

    let (body, status) = reply(&*h, &modern("tools/call", json!({ "name": "list_groups", "arguments": {} })));
    assert_eq!(status, 200);
    assert_eq!(body["result"]["resultType"], "complete");
    assert_eq!(body["result"]["content"][0]["type"], "text");
    assert!(body["result"]["_meta"]["io.modelcontextprotocol/serverInfo"].is_object());

    // what is wrong with a request, and the HTTP status that goes with it
    let code = |msg: Value| {
        let (body, status) = reply(&*h, &msg);
        assert!(body.get("result").is_none(), "{body}");
        (body["error"]["code"].as_i64().unwrap(), status)
    };
    assert_eq!(code(modern("resources/list", json!({}))), (-32601, 404));
    assert_eq!(code(modern("ping", json!({}))), (-32601, 404), "the stateless versions have no ping");
    assert_eq!(code(modern("tools/call", json!({ "name": "format_disk", "arguments": {} }))), (-32602, 400));
    assert_eq!(code(modern("tools/call", json!({ "arguments": {} }))), (-32602, 400));
    assert_eq!(code(modern("tools/call", json!({ "name": "list_groups", "arguments": [1] }))), (-32602, 400));
    assert_eq!(code(modern("tools/list", json!({ "cursor": "abc" }))), (-32602, 400));
    let mut no_caps = modern("tools/list", json!({}));
    no_caps["params"]["_meta"].as_object_mut().unwrap().remove(proto::META_CAPABILITIES);
    assert_eq!(code(no_caps), (-32602, 400));
    assert_eq!(code(json!({ "jsonrpc": "2.0", "id": 1, "method": "server/discover" })), (-32602, 400), "discover says its version");
    assert_eq!(code(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": [] })), (-32602, 400));

    // a version this server does not speak: which ones it does
    let mut future = modern("tools/list", json!({}));
    future["params"]["_meta"][proto::META_VERSION] = json!("2030-01-01");
    let (body, status) = reply(&*h, &future);
    assert_eq!((body["error"]["code"].as_i64(), status), (Some(-32022), 400));
    assert_eq!(body["error"]["data"]["requested"], "2030-01-01");
    assert_eq!(body["error"]["data"]["supported"][0], MODERN);
    // an older version in `_meta`: those open with initialize
    future["params"]["_meta"][proto::META_VERSION] = json!("2025-11-25");
    assert_eq!(reply(&*h, &future).0["error"]["code"], -32022);
}

#[test]
fn messages_that_are_not_requests() {
    let h = host("shapes");
    assert_eq!(proto::handle(&*h, &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })), Outcome::Accepted);
    assert_eq!(proto::handle(&*h, &json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": 3 } })), Outcome::Accepted);
    assert_eq!(proto::handle(&*h, &json!({ "jsonrpc": "2.0", "method": "tools/call", "params": { "name": "add_group", "arguments": { "name": "x" } } })), Outcome::Accepted, "no id: no call");
    assert!(h.store.snapshot()["groups"].as_object().is_none_or(Map::is_empty), "a notification does nothing");
    assert_eq!(proto::handle(&*h, &json!({ "jsonrpc": "2.0", "id": 1, "result": {} })), Outcome::Accepted, "a response: nothing was asked");
    for bad in [json!([{ "jsonrpc": "2.0", "id": 1, "method": "ping" }]), json!("ping"), json!(null), json!({ "jsonrpc": "2.0", "id": 1 }), json!({ "jsonrpc": "2.0", "id": null, "method": "ping" }), json!({ "jsonrpc": "2.0", "id": { "a": 1 }, "method": "ping" })] {
        let (body, status) = reply(&*h, &bad);
        assert_eq!((body["error"]["code"].as_i64(), status), (Some(-32600), 400), "{bad}");
    }
}

#[test]
fn the_handshake_of_the_older_versions() {
    let h = host("legacy");
    let init = |version: &str| reply(&*h, &legacy("initialize", json!({ "protocolVersion": version, "capabilities": {}, "clientInfo": { "name": "old", "version": "1" } })));
    let (body, status) = init("2025-06-18");
    assert_eq!(status, 200);
    let r = &body["result"];
    assert_eq!(r["protocolVersion"], "2025-06-18", "the version asked for");
    assert_eq!((r["serverInfo"]["name"].as_str(), r["serverInfo"]["version"].as_str()), (Some("fluentcal"), Some(env!("CARGO_PKG_VERSION"))));
    assert!(r["capabilities"]["tools"].is_object() && r["instructions"].is_string());
    assert!(r.get("resultType").is_none() && r.get("_meta").is_none(), "nothing the older versions do not know");
    assert_eq!(init("2024-11-05").0["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init("1999-01-01").0["result"]["protocolVersion"], "2025-11-25", "unknown: the newest of the older ones");
    assert_eq!(init(MODERN).0["result"]["protocolVersion"], "2025-11-25", "initialize is the older way, whatever it asks for");

    assert_eq!(reply(&*h, &legacy("ping", json!({}))), (json!({ "jsonrpc": "2.0", "id": "a-1", "result": {} }), 200));
    assert_eq!(reply(&*h, &json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" })).0["result"], json!({}), "no params at all");
    let (body, _) = reply(&*h, &legacy("tools/list", json!({})));
    assert_eq!(body["result"]["tools"].as_array().unwrap().len(), 11);
    assert!(body["result"].get("ttlMs").is_none() && body["result"].get("resultType").is_none());
    let (body, status) = reply(&*h, &legacy("resources/list", json!({})));
    assert_eq!((body["error"]["code"].as_i64(), status), (Some(-32601), 200), "the older versions carry errors in a 200");
    let (body, status) = reply(&*h, &legacy("tools/call", json!({ "name": "nope" })));
    assert_eq!((body["error"]["code"].as_i64(), status), (Some(-32602), 200));
    assert!(body["error"]["message"].as_str().unwrap().contains("nope"));
}

#[test]
fn every_tool_is_described() {
    let list = tools::list();
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 11);
    for t in list {
        let name = t["name"].as_str().unwrap();
        assert!(tools::exists(name), "{name}");
        assert!(name.len() <= 64 && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'), "{name}");
        assert!(t["title"].as_str().is_some_and(|s| !s.is_empty()) && t["description"].as_str().is_some_and(|s| s.len() > 40), "{name}");
        assert!(t["description"].as_str().unwrap().is_ascii() || name == "list_tasks", "{name}: descriptions are English");
        let schema = &t["inputSchema"];
        assert_eq!((schema["type"].as_str(), schema["additionalProperties"].as_bool()), (Some("object"), Some(false)), "{name}");
        assert!(schema["properties"].is_object(), "{name}");
        for required in schema["required"].as_array().into_iter().flatten() {
            assert!(schema["properties"].get(required.as_str().unwrap()).is_some(), "{name}: required {required} is not a property");
        }
        for (key, p) in schema["properties"].as_object().unwrap() {
            assert!(p["type"].is_string() && p["description"].is_string(), "{name}.{key}");
        }
        let read_only = t["annotations"]["readOnlyHint"].as_bool().unwrap();
        assert_eq!(read_only, matches!(name, "list_tasks" | "search_tasks" | "get_task" | "list_groups" | "list_todos" | "get_days"), "{name}");
        assert_eq!(t["annotations"]["openWorldHint"], false, "{name}");
        if !read_only {
            assert_eq!(t["annotations"]["destructiveHint"].as_bool(), Some(matches!(name, "update_task" | "delete_task")), "{name}");
        }
    }
    assert!(!tools::exists("delete_group") && !tools::exists("") && !tools::exists("settings"));
    // an argument nobody knows is told, not ignored
    let h = host("unknown-arg");
    assert!(err(&*h, "list_tasks", json!({ "form": "2026-11-01" })).contains("\"form\""));
    assert!(err(&*h, "list_groups", json!({ "x": 1 })).contains("no arguments"));
}

// ---------------------------------------------------------------------------------------
// The tools that look
// ---------------------------------------------------------------------------------------

#[test]
fn lists_what_is_on_the_calendar() {
    let h = host("list");
    put_group(&h.store, "g1", "工作", 1);
    put(&h.store, json!({ "id": "a", "title": "写周报", "start": "2026-11-18", "end": "2026-11-18", "time": "14:00", "groupId": "g1", "order": 2, "notes": "发给组长" }));
    put(&h.store, json!({ "id": "b", "title": "出差", "start": "2026-11-17", "end": "2026-11-19", "order": 1, "progress": 40 }));
    put(&h.store, json!({ "id": "c", "title": "晨跑", "start": "2026-11-16", "end": "2026-11-16", "time": "07:00", "order": 3, "repeat": { "freq": "daily", "interval": 1 }, "doneDates": ["2026-11-18"] }));
    put(&h.store, json!({ "id": "d", "title": "已经做完", "start": "2026-11-18", "end": "2026-11-18", "done": true, "progress": 100, "order": 4 }));
    put(&h.store, json!({ "id": "e", "title": "下个月", "start": "2026-12-20", "end": "2026-12-20", "order": 5 }));

    let told = call(&*h, "list_tasks", json!({ "from": "2026-11-18", "to": "2026-11-19" })).unwrap();
    assert_eq!(told["today"], today());
    assert_eq!((told["from"].as_str(), told["to"].as_str()), (Some("2026-11-18"), Some("2026-11-19")));
    let rows = told["tasks"].as_array().unwrap();
    let seen: Vec<(&str, &str)> = rows.iter().map(|r| (r["date"].as_str().unwrap(), r["id"].as_str().unwrap())).collect();
    // by day, then time of day (all day first), then the user's order
    assert_eq!(seen, [("2026-11-17", "b"), ("2026-11-18", "d"), ("2026-11-18", "c"), ("2026-11-18", "a"), ("2026-11-19", "c")]);
    assert_eq!(told["count"], 5);
    let row = |id: &str, date: &str| rows.iter().find(|r| r["id"] == id && r["date"] == date).unwrap();
    assert_eq!(row("b", "2026-11-17")["end"], "2026-11-19");
    assert_eq!(row("b", "2026-11-17")["progress"], 40);
    assert_eq!((row("a", "2026-11-18")["time"].as_str(), row("a", "2026-11-18")["group"].as_str(), row("a", "2026-11-18")["notes"].as_str()), (Some("14:00"), Some("工作"), Some("发给组长")));
    assert!(row("a", "2026-11-18").get("end").is_none(), "one day: no end");
    assert_eq!((row("c", "2026-11-18")["done"].as_bool(), row("c", "2026-11-19")["done"].as_bool()), (Some(true), Some(false)), "each occurrence its own tick");
    assert_eq!(row("c", "2026-11-19")["repeats"], "每天");

    let ids = |args: Value| -> Vec<String> { call(&*h, "list_tasks", args).unwrap()["tasks"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap().to_owned()).collect() };
    assert_eq!(ids(json!({ "from": "2026-11-18", "to": "2026-11-18", "status": "open" })), ["b", "a"]);
    assert_eq!(ids(json!({ "from": "2026-11-18", "to": "2026-11-18", "status": "done" })), ["d", "c"]);
    assert_eq!(ids(json!({ "from": "2026-11-01", "to": "2026-12-31", "group": "工作" })), ["a"]);
    assert_eq!(ids(json!({ "from": "2026-11-01", "to": "2026-12-31", "group": "g1" })), ["a"], "by id too");
    assert_eq!(ids(json!({ "from": "2026-12-20" })), ["e", "c", "c", "c", "c", "c", "c", "c"], "seven days from `from`");
    let short = call(&*h, "list_tasks", json!({ "from": "2026-11-18", "to": "2026-11-30", "limit": 3 })).unwrap();
    assert_eq!((short["tasks"].as_array().unwrap().len(), short["truncated"].as_bool(), short["count"].as_u64()), (3, Some(true), Some(16)));
    // without arguments: today and the six days after
    let week = call(&*h, "list_tasks", json!({})).unwrap();
    assert_eq!((week["from"].as_str().unwrap(), week["to"].as_str().unwrap()), (today().as_str(), add_days(&today(), 6).as_str()));
    assert!(["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"].contains(&week["weekday"].as_str().unwrap()));

    assert!(err(&*h, "list_tasks", json!({ "from": "2026-11-18", "to": "2026-11-01" })).contains("before"));
    assert!(err(&*h, "list_tasks", json!({ "from": "2026-01-01", "to": "2027-06-01" })).contains("366"));
    assert!(err(&*h, "list_tasks", json!({ "from": "18/11/2026" })).contains("YYYY-MM-DD"));
    assert!(err(&*h, "list_tasks", json!({ "from": "2026-02-30" })).contains("YYYY-MM-DD"), "not a day");
    assert!(err(&*h, "list_tasks", json!({ "status": "pending" })).contains("all, open, done"));
    assert!(err(&*h, "list_tasks", json!({ "limit": 0 })).contains("1 to 500"));
    assert!(err(&*h, "list_tasks", json!({ "group": "学习" })).contains("\"工作\""), "which groups there are");
}

#[test]
fn finds_reads_and_counts() {
    let h = host("read");
    put_group(&h.store, "g1", "工作", 1);
    put_group(&h.store, "g2", "家", 2);
    put(&h.store, json!({ "id": "a", "title": "年终总结", "start": "2026-12-28", "end": "2026-12-30", "groupId": "g1", "order": 1, "notes": "第一行\n第二行", "reminders": [0, 1440], "color": "red", "deps": ["b"] }));
    put(&h.store, json!({ "id": "b", "title": "收集数据", "start": "2026-12-20", "end": "2026-12-22", "groupId": "g1", "order": 2, "done": true, "progress": 100 }));
    put(&h.store, json!({ "id": "c", "title": "交房租", "start": "2026-01-05", "end": "2026-01-05", "groupId": "g2", "order": 3, "repeat": { "freq": "monthly", "interval": 1 }, "exDates": ["2026-03-05"] }));
    put(&h.store, json!({ "id": "d", "title": "没有分组", "start": "2026-11-18", "end": "2026-11-18", "order": 4, "groupId": "gone" }));

    let found = call(&*h, "search_tasks", json!({ "query": "总结" })).unwrap();
    assert_eq!((found["count"].as_u64(), found["tasks"][0]["id"].as_str()), (Some(1), Some("a")));
    assert_eq!(call(&*h, "search_tasks", json!({ "query": "第二行" })).unwrap()["count"], 1, "in the notes");
    let work = call(&*h, "search_tasks", json!({ "query": "工作" })).unwrap();
    assert_eq!(work["count"], 2, "by the group's name");
    assert_eq!(call(&*h, "search_tasks", json!({ "query": "工作", "status": "open" })).unwrap()["count"], 1);
    assert_eq!(call(&*h, "search_tasks", json!({ "group": "家" })).unwrap()["tasks"][0]["repeats"], "每个月的 5 日");
    assert_eq!(call(&*h, "search_tasks", json!({ "limit": 2 })).unwrap()["truncated"], true);
    assert_eq!(call(&*h, "search_tasks", json!({ "query": "不存在的词" })).unwrap()["count"], 0);

    let a = call(&*h, "get_task", json!({ "id": "a" })).unwrap();
    assert_eq!((a["start"].as_str(), a["end"].as_str(), a["notes"].as_str()), (Some("2026-12-28"), Some("2026-12-30"), Some("第一行\n第二行")));
    assert_eq!(a["group"], json!({ "id": "g1", "name": "工作" }));
    assert_eq!((a["reminders"].clone(), a["color"].as_str(), a["done"].as_bool(), a["progress"].as_i64()), (json!([0, 1440]), Some("red"), Some(false), Some(0)));
    assert_eq!(a["dependsOn"], json!([{ "id": "b", "title": "收集数据" }]));
    let c = call(&*h, "get_task", json!({ "id": "c" })).unwrap();
    assert_eq!(c["repeat"]["freq"], "monthly");
    assert_eq!(c["skipped"], json!(["2026-03-05"]));
    let next = c["nextOccurrences"].as_array().unwrap();
    assert_eq!(next.len(), 5);
    assert!(next.iter().all(|o| o["date"].as_str().unwrap().ends_with("-05") && o["date"].as_str().unwrap() >= today().as_str()));
    assert!(c.get("done").is_none(), "a repeating task is ticked by occurrence");
    assert!(err(&*h, "get_task", json!({ "id": "zzz" })).contains("zzz"));
    assert!(err(&*h, "get_task", json!({})).contains("required"));

    let groups = call(&*h, "list_groups", json!({})).unwrap();
    assert_eq!(groups["groups"], json!([{ "id": "g1", "name": "工作", "color": "blue", "tasks": 2, "open": 1 }, { "id": "g2", "name": "家", "color": "blue", "tasks": 1, "open": 1 }]));
    assert_eq!(groups["withoutGroup"], json!({ "tasks": 1, "open": 1 }), "a group that is gone counts as none");
}

#[test]
fn tells_about_days() {
    let h = host("days");
    let told = call(&*h, "get_days", json!({ "from": "2026-10-01", "to": "2026-10-10" })).unwrap();
    let days = told["days"].as_array().unwrap();
    assert_eq!(days.len(), 10);
    let day = |d: &str| days.iter().find(|x| x["date"] == d).unwrap();
    assert_eq!((day("2026-10-01")["weekday"].as_str(), day("2026-10-01")["holiday"].as_str(), day("2026-10-01")["holidayKind"].as_str(), day("2026-10-01")["workday"].as_bool()), (Some("Thursday"), Some("国庆节"), Some("off"), Some(false)));
    assert!(day("2026-10-01")["festivals"].as_array().unwrap().contains(&json!("国庆节")));
    assert_eq!((day("2026-10-10")["weekday"].as_str(), day("2026-10-10")["holidayKind"].as_str(), day("2026-10-10")["workday"].as_bool()), (Some("Saturday"), Some("work"), Some(true)), "a Saturday moved to work");
    assert_eq!((day("2026-10-09")["workday"].as_bool(), day("2026-10-09").get("holiday")), (Some(true), None));
    assert_eq!(day("2026-10-08")["solarTerm"], "寒露");
    assert!(day("2026-10-09")["lunar"].as_str().is_some_and(|s| !s.is_empty()));

    let only = |which: &str| -> Vec<String> { call(&*h, "get_days", json!({ "from": "2026-10-01", "to": "2026-10-12", "only": which })).unwrap()["days"].as_array().unwrap().iter().map(|d| d["date"].as_str().unwrap().to_owned()).collect() };
    let holidays = only("holidays");
    assert!(holidays.contains(&"2026-10-01".to_owned()) && holidays.contains(&"2026-10-10".to_owned()) && !holidays.contains(&"2026-10-09".to_owned()) && !holidays.contains(&"2026-10-11".to_owned()));
    assert!(only("special").contains(&"2026-10-08".to_owned()) && !only("special").contains(&"2026-10-09".to_owned()));
    // the user's own weekend
    h.store.patch_settings(&json!({ "calendar": { "restDays": [5] } }));
    assert_eq!(call(&*h, "get_days", json!({ "from": "2026-11-20" })).unwrap()["days"][0]["workday"], false, "Friday off");
    assert_eq!(call(&*h, "get_days", json!({ "from": "2026-11-21" })).unwrap()["days"][0]["workday"], true);

    let now = call(&*h, "get_days", json!({})).unwrap();
    assert_eq!((now["days"].as_array().unwrap().len(), now["days"][0]["date"].as_str().unwrap()), (1, today().as_str()));
    let year = call(&*h, "get_days", json!({ "from": "2026-01-01", "to": "2026-12-31" })).unwrap();
    assert_eq!((year["days"].as_array().unwrap().len(), year["truncated"].as_bool()), (120, Some(true)));
    assert!(err(&*h, "get_days", json!({ "from": "2026-01-01", "to": "2028-01-01" })).contains("400"));
    assert!(err(&*h, "get_days", json!({ "only": "weekends" })).contains("all, special, holidays"));
}

// ---------------------------------------------------------------------------------------
// The tools that change
// ---------------------------------------------------------------------------------------

#[test]
fn adds_tasks_by_the_editors_rules() {
    let h = host("add");
    put_group(&h.store, "g1", "工作", 1);
    put(&h.store, json!({ "id": "old", "title": "已有", "start": "2026-11-01", "end": "2026-11-01", "order": 7 }));

    let id = add(&*h, json!({ "title": "  写报告\n ", "start": "2026-11-20" }));
    let t = stored(&h, &id);
    assert_eq!((t["title"].as_str(), t["start"].as_str(), t["end"].as_str(), t["order"].as_i64()), (Some("写报告"), Some("2026-11-20"), Some("2026-11-20"), Some(8)));
    assert!(t["createdAt"].as_i64().unwrap() > 0 && t["createdAt"] == t["updatedAt"]);
    assert!(t.get("color").is_none() && t.get("reminders").is_none() && t.get("time").is_none());

    let id = add(&*h, json!({ "title": "评审", "start": "2026-11-20", "end": "2026-11-22", "time": "9:30", "notes": "带电脑", "color": "RED", "group": "工作", "countdown": true, "reminders": [1440, 0, 0], "progress": 30 }));
    let t = stored(&h, &id);
    assert_eq!((t["end"].as_str(), t["time"].as_str(), t["notes"].as_str(), t["color"].as_str(), t["groupId"].as_str()), (Some("2026-11-22"), Some("09:30"), Some("带电脑"), Some("red"), Some("g1")));
    assert_eq!((t["countdown"].as_bool(), t["reminders"].clone(), t["progress"].as_i64(), t["done"].as_bool()), (Some(true), json!([0, 1440]), Some(30), Some(false)));
    assert_eq!(stored(&h, &add(&*h, json!({ "title": "做完的", "start": "2026-11-20", "done": true })))["progress"], 100);
    assert_eq!(stored(&h, &add(&*h, json!({ "title": "里程碑", "start": "2026-11-20", "end": "2026-11-25", "milestone": true, "color": "#1a2b3c" }))), {
        let mut t = stored(&h, &call(&*h, "search_tasks", json!({ "query": "里程碑" })).unwrap()["tasks"][0]["id"].as_str().unwrap().to_owned());
        assert_eq!((t["end"].as_str(), t["milestone"].as_bool(), t["color"].as_str()), (Some("2026-11-20"), Some(true), Some("#1A2B3C")), "a milestone is one day");
        t.take()
    });

    // the user's defaults, unless the call says otherwise
    h.store.patch_settings(&json!({ "tasks": { "defaultColor": "green" }, "reminders": { "defaultOffsets": [10] } }));
    let t = stored(&h, &add(&*h, json!({ "title": "默认", "start": "2026-11-21" })));
    assert_eq!((t["color"].as_str(), t["reminders"].clone()), (Some("green"), json!([10])));
    let t = stored(&h, &add(&*h, json!({ "title": "不要默认", "start": "2026-11-21", "color": "", "reminders": [] })));
    assert!(t.get("color").is_none() && t.get("reminders").is_none(), "{t}");

    // what is wrong is said, and nothing is added
    let before = count(&h);
    for (task, why) in [
        (json!({ "start": "2026-11-20" }), "\"title\" is required"),
        (json!({ "title": "   ", "start": "2026-11-20" }), "cannot be empty"),
        (json!({ "title": "x" }), "\"start\""),
        (json!({ "title": "x", "start": "明天" }), "YYYY-MM-DD"),
        (json!({ "title": "x", "start": "2026-11-20", "end": "2026-11-19" }), "before \"start\""),
        (json!({ "title": "x", "start": "2026-11-20", "time": "25:00" }), "HH:MM"),
        (json!({ "title": "x", "start": "2026-11-20", "time": "下午三点" }), "HH:MM"),
        (json!({ "title": "x", "start": "2026-11-20", "color": "crimson" }), "red, orange"),
        (json!({ "title": "x", "start": "2026-11-20", "group": "学习" }), "\"工作\""),
        (json!({ "title": "x", "start": "2026-11-20", "reminders": [-5] }), "minutes"),
        (json!({ "title": "x", "start": "2026-11-20", "reminders": [1, 2, 3, 4, 5, 6] }), "five"),
        (json!({ "title": "x", "start": "2026-11-20", "progress": 150 }), "0 to 100"),
        (json!({ "title": "x", "start": "2026-11-20", "priority": "high" }), "\"priority\""),
        (json!({ "title": "x", "start": "2026-11-20", "dependsOn": ["nobody"] }), "nobody"),
        (json!({ "title": "x", "start": "2026-11-20", "milestone": "yes" }), "true or false"),
        (json!("买菜"), "must be an object"),
    ] {
        let said = err(&*h, "add_tasks", json!({ "tasks": [task.clone()] }));
        assert!(said.contains(why), "{task}: {said}");
    }
    assert!(err(&*h, "add_tasks", json!({})).contains("required"));
    assert!(err(&*h, "add_tasks", json!({ "tasks": [] })).contains("at least one"));
    assert!(err(&*h, "add_tasks", json!({ "tasks": vec![json!({ "title": "x", "start": "2026-11-20" }); 51] })).contains("50"));
    // several: all or none, and which one was wrong
    let said = err(&*h, "add_tasks", json!({ "tasks": [{ "title": "好的", "start": "2026-11-20" }, { "title": "坏的", "start": "2026-13-01" }] }));
    assert!(said.starts_with("tasks[1]:"), "{said}");
    assert_eq!(count(&h), before, "nothing was added");
}

#[test]
fn adds_several_tasks_with_predecessors() {
    let h = host("add-many");
    put(&h.store, json!({ "id": "old", "title": "已有", "start": "2026-11-01", "end": "2026-11-01", "order": 3 }));
    let told = call(&*h, "add_tasks", json!({ "tasks": [
        { "title": "设计", "start": "2026-12-01", "end": "2026-12-03", "ref": "design" },
        { "title": "开发", "start": "2026-12-04", "end": "2026-12-10", "ref": "dev", "dependsOn": ["design", "old"] },
        { "title": "上线", "start": "2026-12-11", "milestone": true, "dependsOn": ["dev"] }
    ] })).unwrap();
    assert_eq!(told["added"], 3);
    let ids: Vec<&str> = told["tasks"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!((told["tasks"][0]["ref"].as_str(), told["tasks"][2].get("ref")), (Some("design"), None));
    assert_eq!(stored(&h, ids[1])["deps"], json!([ids[0], "old"]));
    assert_eq!(stored(&h, ids[2])["deps"], json!([ids[1]]));
    assert_eq!((stored(&h, ids[0])["order"].as_i64(), stored(&h, ids[1])["order"].as_i64(), stored(&h, ids[2])["order"].as_i64()), (Some(4), Some(5), Some(6)), "in the order given, after what is there");

    assert!(err(&*h, "add_tasks", json!({ "tasks": [{ "title": "a", "start": "2026-12-01", "dependsOn": ["later"] }, { "title": "b", "start": "2026-12-02", "ref": "later" }] })).contains("earlier in this call"));
    assert!(err(&*h, "add_tasks", json!({ "tasks": [{ "title": "a", "start": "2026-12-01", "ref": "x" }, { "title": "b", "start": "2026-12-02", "ref": "x" }] })).contains("twice"));
    assert_eq!(count(&h), 4);
}

#[test]
fn adds_repeating_tasks() {
    let h = host("repeat");
    let rule = |task: Value| stored(&h, &add(&*h, task));
    // 2026-11-18 is a Wednesday
    let t = rule(json!({ "title": "周会", "start": "2026-11-18", "repeat": { "freq": "weekly" } }));
    assert_eq!(t["repeat"], json!({ "freq": "weekly", "interval": 1, "weekdays": [3] }), "the weekday of the start");
    let t = rule(json!({ "title": "健身", "start": "2026-11-18", "repeat": { "freq": "weekly", "interval": 2, "weekdays": [5, 1, 5], "count": 10 } }));
    assert_eq!(t["repeat"], json!({ "freq": "weekly", "interval": 2, "weekdays": [1, 5], "count": 10 }));
    let t = rule(json!({ "title": "例会", "start": "2026-11-18", "repeat": { "freq": "monthly", "monthMode": "nthWeekday", "until": "2027-06-30" } }));
    assert_eq!(t["repeat"], json!({ "freq": "monthly", "interval": 1, "monthMode": "nthWeekday", "until": "2027-06-30" }));
    // "every day from the 18th to the 22nd" is one day, daily, until the 22nd (GitHub #12)
    let t = rule(json!({ "title": "吃药", "start": "2026-11-18", "end": "2026-11-22", "repeat": { "freq": "daily" } }));
    assert_eq!((t["end"].as_str(), t["repeat"]["until"].as_str()), (Some("2026-11-18"), Some("2026-11-22")));
    for freq in ["yearly", "workday", "legalWorkday", "lunarYearly", "monthLastDay", "ebbinghaus"] {
        assert_eq!(rule(json!({ "title": freq, "start": "2026-11-18", "repeat": { "freq": freq } }))["repeat"]["freq"], freq);
    }
    let listed = call(&*h, "list_tasks", json!({ "from": "2026-11-25", "to": "2026-11-25" })).unwrap();
    assert!(listed["tasks"].as_array().unwrap().iter().any(|r| r["title"] == "周会" && r["repeats"] == "每周的周三"), "{listed}");

    let before = count(&h);
    for (repeat, extra, why) in [
        (json!({ "freq": "fortnightly" }), json!({}), "daily, weekly"),
        (json!({}), json!({}), "daily, weekly"),
        (json!("weekly"), json!({}), "must be an object"),
        (json!({ "freq": "weekly", "weekdays": [7] }), json!({}), "0 (Sunday) to 6"),
        (json!({ "freq": "daily", "weekdays": [1] }), json!({}), "only goes with weekly"),
        (json!({ "freq": "weekly", "monthMode": "day" }), json!({}), "only goes with monthly"),
        (json!({ "freq": "daily", "count": 3, "until": "2026-12-01" }), json!({}), "not both"),
        (json!({ "freq": "daily", "until": "2026-11-01" }), json!({}), "before \"start\""),
        (json!({ "freq": "daily", "interval": 0 }), json!({}), "1 to 99"),
        (json!({ "freq": "workday", "interval": 2 }), json!({}), "does not go with workday"),
        (json!({ "freq": "ebbinghaus", "count": 3 }), json!({}), "seven review days"),
        (json!({ "freq": "daily", "every": 2 }), json!({}), "repeat: unknown argument \"every\""),
        (json!({ "freq": "daily" }), json!({ "done": true }), "one occurrence at a time"),
        (json!({ "freq": "daily" }), json!({ "progress": 50 }), "one occurrence at a time"),
    ] {
        let mut task = json!({ "title": "x", "start": "2026-11-18", "repeat": repeat });
        for (k, v) in extra.as_object().unwrap() {
            task[k] = v.clone();
        }
        let said = err(&*h, "add_tasks", json!({ "tasks": [task.clone()] }));
        assert!(said.contains(why), "{task}: {said}");
    }
    let first = add(&*h, json!({ "title": "前置", "start": "2026-11-18" }));
    assert!(err(&*h, "add_tasks", json!({ "tasks": [{ "title": "x", "start": "2026-11-19", "repeat": { "freq": "daily" }, "dependsOn": [first] }] })).contains("cannot come after"));
    let weekly = call(&*h, "search_tasks", json!({ "query": "周会" })).unwrap()["tasks"][0]["id"].as_str().unwrap().to_owned();
    assert!(err(&*h, "add_tasks", json!({ "tasks": [{ "title": "x", "start": "2026-11-19", "dependsOn": [weekly] }] })).contains("repeats"));
    assert_eq!(count(&h), before + 1);
}

#[test]
fn todos_without_a_day() {
    let h = host("todos");
    let now = today();
    let told = call(&*h, "add_tasks", json!({ "tasks": [{ "title": "交水电费", "todo": "monthly", "notes": "网上交" }, { "title": "学游泳", "todo": "someday", "color": "teal" }] })).unwrap();
    let (monthly, someday) = (told["tasks"][0]["id"].as_str().unwrap().to_owned(), told["tasks"][1]["id"].as_str().unwrap().to_owned());
    assert_eq!((told["tasks"][0]["todo"].as_str(), told["tasks"][1]["todo"].as_str()), (Some("monthly"), Some("someday")));
    assert!(told["tasks"][0].get("date").is_none(), "no day");
    assert_eq!((stored(&h, &monthly)["flex"].as_str(), stored(&h, &monthly)["start"].as_str().unwrap(), stored(&h, &monthly)["autoPostpone"].as_bool()), (Some("month"), start_of_month(&now).as_str(), Some(false)));
    assert_eq!((stored(&h, &someday)["flex"].as_str(), stored(&h, &someday)["start"].as_str().unwrap()), (Some("someday"), now.as_str()));
    for (field, value) in [("start", json!("2026-11-20")), ("time", json!("09:00")), ("repeat", json!({ "freq": "daily" })), ("done", json!(true)), ("milestone", json!(true))] {
        let mut task = json!({ "title": "x", "todo": "someday" });
        task[field] = value;
        assert!(err(&*h, "add_tasks", json!({ "tasks": [task] })).contains("to-do without a day"), "{field}");
    }
    assert!(err(&*h, "add_tasks", json!({ "tasks": [{ "title": "x", "todo": "weekly" }] })).contains("monthly, someday"));

    let todos = call(&*h, "list_todos", json!({})).unwrap();
    assert_eq!((todos["month"].as_str().unwrap(), todos["today"].as_str().unwrap()), (&now[..7], now.as_str()));
    assert_eq!(todos["monthly"], json!([{ "id": monthly, "title": "交水电费", "notes": "网上交", "done": false }]));
    assert_eq!(todos["someday"], json!([{ "id": someday, "title": "学游泳", "color": "teal" }]));
    assert_eq!(todos["overdue"], json!([]));
    assert_eq!(call(&*h, "search_tasks", json!({ "query": "游泳" })).unwrap()["tasks"][0]["todo"], "someday");
    assert_eq!(call(&*h, "list_tasks", json!({})).unwrap()["count"], 0, "not on the calendar while open");

    // ticking: this month's slot, on the day it was done
    let told = call(&*h, "set_task_done", json!({ "id": monthly })).unwrap();
    assert_eq!((told["done"].as_bool(), told["doneOn"].as_str().unwrap(), told["month"].as_str().unwrap()), (Some(true), now.as_str(), &now[..7]));
    assert_eq!(stored(&h, &monthly)["flexDone"], json!({ (&now[..7]): now }));
    assert_eq!(call(&*h, "list_todos", json!({})).unwrap()["monthly"][0]["doneOn"], now);
    assert_eq!(call(&*h, "list_tasks", json!({})).unwrap()["tasks"][0]["todo"], "monthly", "on the calendar on the day it was done");
    assert_eq!(call(&*h, "get_task", json!({ "id": monthly })).unwrap()["doneThisMonth"], true);
    assert_eq!(call(&*h, "set_task_done", json!({ "id": monthly })).unwrap()["unchanged"], true);
    assert_eq!(call(&*h, "set_task_done", json!({ "id": monthly, "done": false })).unwrap()["done"], false);
    assert!(stored(&h, &monthly).get("flexDone").is_none());
    assert!(err(&*h, "set_task_done", json!({ "id": monthly, "date": add_days(&now, 40) })).contains("has not come"));
    assert!(err(&*h, "set_task_done", json!({ "id": monthly, "date": "2020-01-15" })).contains("only began"));

    call(&*h, "set_task_done", json!({ "id": someday })).unwrap();
    assert_eq!((stored(&h, &someday)["done"].as_bool(), stored(&h, &someday)["flexDone"].clone()), (Some(true), json!({ "": now })));
    assert_eq!(call(&*h, "list_todos", json!({})).unwrap()["someday"], json!([]), "done: off the list");
    assert_eq!(call(&*h, "get_task", json!({ "id": someday })).unwrap()["doneOn"], now);

    // a to-do takes few fields
    assert_eq!(call(&*h, "update_task", json!({ "id": monthly, "title": "交水电燃气费", "notes": "" })).unwrap()["task"]["title"], "交水电燃气费");
    assert!(stored(&h, &monthly).get("notes").is_none());
    assert!(err(&*h, "update_task", json!({ "id": monthly, "start": "2026-11-20" })).contains("to-do without a day"));
    assert!(err(&*h, "update_task", json!({ "id": someday, "done": false })).contains("set_task_done"));
    call(&*h, "delete_task", json!({ "id": someday })).unwrap();
    assert!(stored(&h, &someday).is_null());
}

#[test]
fn changes_a_task() {
    let h = host("update");
    put_group(&h.store, "g1", "工作", 1);
    put(&h.store, json!({ "id": "a", "title": "写方案", "start": "2026-11-18", "end": "2026-11-20", "time": "10:00", "notes": "初稿", "color": "red", "groupId": "g1", "reminders": [10], "order": 1, "createdAt": 5, "updatedAt": 5, "futureField": { "kept": true } }));
    put(&h.store, json!({ "id": "b", "title": "评审", "start": "2026-11-21", "end": "2026-11-21", "order": 2, "deps": ["a"] }));
    put(&h.store, json!({ "id": "c", "title": "上线", "start": "2026-11-25", "end": "2026-11-25", "order": 3, "deps": ["b"] }));

    let told = call(&*h, "update_task", json!({ "id": "a", "title": "写技术方案" })).unwrap();
    assert_eq!(told["task"]["title"], "写技术方案");
    let t = stored(&h, "a");
    assert_eq!((t["title"].as_str(), t["start"].as_str(), t["time"].as_str(), t["createdAt"].as_i64()), (Some("写技术方案"), Some("2026-11-18"), Some("10:00"), Some(5)), "only what was given");
    assert!(t["updatedAt"].as_i64().unwrap() > 5);
    assert_eq!(t["futureField"], json!({ "kept": true }), "what this version does not know stays");

    // moving keeps the length; an end of its own sets it
    call(&*h, "update_task", json!({ "id": "a", "start": "2026-12-01" })).unwrap();
    assert_eq!((stored(&h, "a")["start"].as_str(), stored(&h, "a")["end"].as_str()), (Some("2026-12-01"), Some("2026-12-03")));
    call(&*h, "update_task", json!({ "id": "a", "start": "2026-12-05", "end": "2026-12-05" })).unwrap();
    assert_eq!((stored(&h, "a")["start"].as_str(), stored(&h, "a")["end"].as_str()), (Some("2026-12-05"), Some("2026-12-05")));
    call(&*h, "update_task", json!({ "id": "a", "end": "2026-12-09" })).unwrap();
    assert_eq!(stored(&h, "a")["end"], "2026-12-09");
    assert!(err(&*h, "update_task", json!({ "id": "a", "end": "2026-12-01" })).contains("before \"start\""));
    assert!(err(&*h, "update_task", json!({ "id": "a", "start": null })).contains("cannot be removed"));

    // emptied with "", [] or null
    call(&*h, "update_task", json!({ "id": "a", "time": "", "notes": null, "color": "", "group": "", "reminders": [] })).unwrap();
    let t = stored(&h, "a");
    for gone in ["time", "notes", "color", "groupId", "reminders"] {
        assert!(t.get(gone).is_none(), "{gone}: {t}");
    }
    call(&*h, "update_task", json!({ "id": "a", "time": "18:05:00", "group": "g1", "color": "grey" })).unwrap();
    assert_eq!((stored(&h, "a")["time"].as_str(), stored(&h, "a")["groupId"].as_str(), stored(&h, "a")["color"].as_str()), (Some("18:05"), Some("g1"), Some("gray")));

    call(&*h, "update_task", json!({ "id": "a", "progress": 60 })).unwrap();
    assert_eq!((stored(&h, "a")["progress"].as_i64(), stored(&h, "a")["done"].as_bool()), (Some(60), Some(false)));
    call(&*h, "update_task", json!({ "id": "a", "progress": 100 })).unwrap();
    assert_eq!(stored(&h, "a")["done"], true, "100 % is done");
    call(&*h, "update_task", json!({ "id": "a", "done": false })).unwrap();
    assert_eq!((stored(&h, "a")["progress"].as_i64(), stored(&h, "a")["done"].as_bool()), (Some(0), Some(false)));

    // nothing new: nothing written
    let before = stored(&h, "a");
    assert_eq!(call(&*h, "update_task", json!({ "id": "a", "title": "写技术方案" })).unwrap()["unchanged"], true);
    assert_eq!(stored(&h, "a"), before);
    assert!(err(&*h, "update_task", json!({ "id": "a" })).contains("nothing to change"));
    assert!(err(&*h, "update_task", json!({ "id": "nope", "title": "x" })).contains("nope"));
    assert!(err(&*h, "update_task", json!({ "title": "x" })).contains("\"id\" is required"));
    assert!(err(&*h, "update_task", json!({ "id": "a", "title": "" })).contains("cannot be empty"));
    assert!(err(&*h, "update_task", json!({ "id": "a", "todo": "someday" })).contains("\"todo\""));

    // predecessors: no circles, no repeating tasks in a chain
    assert!(err(&*h, "update_task", json!({ "id": "a", "dependsOn": ["c"] })).contains("circle"));
    assert!(err(&*h, "update_task", json!({ "id": "a", "dependsOn": ["a"] })).contains("itself"));
    call(&*h, "update_task", json!({ "id": "c", "dependsOn": ["a", "b", "a"] })).unwrap();
    assert_eq!(stored(&h, "c")["deps"], json!(["a", "b"]));
    call(&*h, "update_task", json!({ "id": "c", "dependsOn": [] })).unwrap();
    assert!(stored(&h, "c").get("deps").is_none());
    assert!(err(&*h, "update_task", json!({ "id": "a", "repeat": { "freq": "daily" } })).contains("come after this one"));
    assert!(err(&*h, "update_task", json!({ "id": "b", "repeat": { "freq": "daily" } })).contains("cannot come after"));
    call(&*h, "update_task", json!({ "id": "c", "repeat": { "freq": "weekly", "count": 4 } })).unwrap();
    assert_eq!(stored(&h, "c")["repeat"], json!({ "freq": "weekly", "interval": 1, "weekdays": [3], "count": 4 }));
    assert!(err(&*h, "update_task", json!({ "id": "c", "progress": 50 })).contains("one occurrence at a time"));
    call(&*h, "update_task", json!({ "id": "c", "repeat": { "freq": "none" } })).unwrap();
    assert!(stored(&h, "c").get("repeat").is_none());
}

#[test]
fn one_occurrence_of_a_repeating_task() {
    let h = host("occurrence");
    // Wednesdays from 2026-11-04
    put(&h.store, json!({ "id": "w", "title": "周会", "start": "2026-11-04", "end": "2026-11-04", "time": "10:00", "order": 4, "color": "blue", "repeat": { "freq": "weekly", "interval": 1, "weekdays": [3] }, "doneDates": ["2026-11-04"] }));
    h.store.patch_settings(&json!({ "tasks": { "defaultColor": "green" } }));

    // ticking
    assert!(err(&*h, "set_task_done", json!({ "id": "w" })).contains("which occurrence"));
    assert!(err(&*h, "set_task_done", json!({ "id": "w", "date": "2026-11-19" })).contains("no occurrence on 2026-11-19"));
    assert_eq!(call(&*h, "set_task_done", json!({ "id": "w", "date": "2026-11-18" })).unwrap()["done"], true);
    assert_eq!(stored(&h, "w")["doneDates"], json!(["2026-11-04", "2026-11-18"]));
    assert_eq!(call(&*h, "set_task_done", json!({ "id": "w", "date": "2026-11-18" })).unwrap()["unchanged"], true);
    call(&*h, "set_task_done", json!({ "id": "w", "date": "2026-11-04", "done": false })).unwrap();
    assert_eq!(stored(&h, "w")["doneDates"], json!(["2026-11-18"]));
    assert_eq!(stored(&h, "w")["done"], false, "the task itself is never done");

    // changing one occurrence takes it out of the series
    assert!(err(&*h, "update_task", json!({ "id": "w", "occurrence": "2026-11-26", "time": "15:00" })).contains("no occurrence"));
    assert!(err(&*h, "update_task", json!({ "id": "w", "occurrence": "2026-11-25", "repeat": { "freq": "daily" } })).contains("does not go with \"occurrence\""));
    let told = call(&*h, "update_task", json!({ "id": "w", "occurrence": "2026-11-25", "start": "2026-11-26", "time": "15:00", "title": "周会（改期）" })).unwrap();
    let copy = told["task"]["id"].as_str().unwrap().to_owned();
    assert_ne!(copy, "w");
    assert_eq!(told["detachedFrom"], "w");
    let c = stored(&h, &copy);
    assert_eq!((c["title"].as_str(), c["start"].as_str(), c["end"].as_str(), c["time"].as_str(), c["order"].as_i64(), c["color"].as_str()), (Some("周会（改期）"), Some("2026-11-26"), Some("2026-11-26"), Some("15:00"), Some(4), Some("blue")));
    assert!(c.get("repeat").is_none() && c.get("doneDates").is_none() && c.get("exDates").is_none());
    assert_eq!(stored(&h, "w")["exDates"], json!(["2026-11-25"]));
    assert_eq!((stored(&h, "w")["title"].as_str(), stored(&h, "w")["time"].as_str()), (Some("周会"), Some("10:00")), "the series stays as it was");
    // a ticked occurrence stays ticked on its own
    let told = call(&*h, "update_task", json!({ "id": "w", "occurrence": "2026-11-18", "notes": "改在线上" })).unwrap();
    assert_eq!((told["task"]["done"].as_bool(), told["task"]["progress"].as_i64()), (Some(true), Some(100)));
    assert!(err(&*h, "update_task", json!({ "id": copy, "occurrence": "2026-11-26", "title": "x" })).contains("does not repeat"));
    // without `occurrence`: the whole series
    call(&*h, "update_task", json!({ "id": "w", "time": "11:00" })).unwrap();
    assert_eq!(stored(&h, "w")["time"], "11:00");

    // deleting: how much has to be said
    assert!(err(&*h, "delete_task", json!({ "id": "w" })).contains("\"scope\""));
    assert!(err(&*h, "delete_task", json!({ "id": "w", "scope": "this" })).contains("needs \"date\""));
    assert!(err(&*h, "delete_task", json!({ "id": "w", "scope": "this", "date": "2026-12-03" })).contains("no occurrence"));
    assert!(err(&*h, "delete_task", json!({ "id": "w", "scope": "some" })).contains("this, future, all"));
    let told = call(&*h, "delete_task", json!({ "id": "w", "scope": "this", "date": "2026-12-02" })).unwrap();
    assert_eq!((told["deleted"].as_str(), told["date"].as_str()), (Some("occurrence"), Some("2026-12-02")));
    assert_eq!(stored(&h, "w")["exDates"], json!(["2026-11-18", "2026-11-25", "2026-12-02"]));
    let told = call(&*h, "delete_task", json!({ "id": "w", "scope": "future", "date": "2026-12-16" })).unwrap();
    assert_eq!(told["deleted"], "occurrencesFrom");
    assert_eq!(stored(&h, "w")["repeat"]["until"], "2026-12-15");
    let dates: Vec<String> = call(&*h, "list_tasks", json!({ "from": "2026-11-01", "to": "2027-01-31" })).unwrap()["tasks"].as_array().unwrap().iter().filter(|r| r["id"] == "w").map(|r| r["date"].as_str().unwrap().to_owned()).collect();
    assert_eq!(dates, ["2026-11-04", "2026-11-11", "2026-12-09"]);
    // from the first occurrence on is the whole task
    put(&h.store, json!({ "id": "d", "title": "每天", "start": "2026-11-10", "end": "2026-11-10", "repeat": { "freq": "daily", "interval": 1 } }));
    assert_eq!(call(&*h, "delete_task", json!({ "id": "d", "scope": "future", "date": "2026-11-10" })).unwrap()["deleted"], "task");
    assert!(stored(&h, "d").is_null());
    assert_eq!(call(&*h, "delete_task", json!({ "id": "w", "scope": "all" })).unwrap()["deleted"], "task");
    assert!(stored(&h, "w").is_null());
    assert!(!stored(&h, &copy).is_null(), "the occurrence taken out lives on");
}

#[test]
fn ticks_deletes_and_groups() {
    let h = host("misc");
    put(&h.store, json!({ "id": "a", "title": "一", "start": "2026-11-18", "end": "2026-11-18", "progress": 40, "order": 1 }));
    put(&h.store, json!({ "id": "b", "title": "二", "start": "2026-11-19", "end": "2026-11-19", "order": 2, "deps": ["a"] }));

    let told = call(&*h, "set_task_done", json!({ "id": "a" })).unwrap();
    assert_eq!(told, json!({ "id": "a", "title": "一", "date": "2026-11-18", "done": true }));
    assert_eq!((stored(&h, "a")["done"].as_bool(), stored(&h, "a")["progress"].as_i64()), (Some(true), Some(100)));
    assert_eq!(call(&*h, "set_task_done", json!({ "id": "a", "done": true })).unwrap()["unchanged"], true);
    call(&*h, "set_task_done", json!({ "id": "a", "done": false })).unwrap();
    assert_eq!((stored(&h, "a")["done"].as_bool(), stored(&h, "a")["progress"].as_i64()), (Some(false), Some(0)));
    assert!(err(&*h, "set_task_done", json!({ "id": "zzz" })).contains("zzz"));
    assert!(err(&*h, "set_task_done", json!({ "id": "a", "done": "yes" })).contains("true or false"));

    // a deleted task is no predecessor any more
    assert_eq!(call(&*h, "delete_task", json!({ "id": "a" })).unwrap(), json!({ "id": "a", "title": "一", "deleted": "task" }));
    assert!(stored(&h, "a").is_null());
    assert_eq!(stored(&h, "b")["deps"], json!([]));
    assert!(err(&*h, "delete_task", json!({ "id": "a" })).contains("no task"));
    assert!(err(&*h, "delete_task", json!({})).contains("required"));

    let g = call(&*h, "add_group", json!({ "name": " 学习 " })).unwrap();
    assert_eq!((g["name"].as_str(), g["color"].as_str(), g.get("existing")), (Some("学习"), Some("red"), None));
    let again = call(&*h, "add_group", json!({ "name": "学习", "color": "green" })).unwrap();
    assert_eq!((again["id"].clone(), again["existing"].as_bool(), again["color"].as_str()), (g["id"].clone(), Some(true), Some("red")), "the one that is there");
    let second = call(&*h, "add_group", json!({ "name": "家务" })).unwrap();
    assert_eq!(second["color"], "orange", "the next colour of the palette");
    assert_eq!(call(&*h, "add_group", json!({ "name": "紫", "color": "#8764b8" })).unwrap()["color"], "#8764B8");
    let groups = h.store.snapshot()["groups"].clone();
    assert_eq!(groups.as_object().unwrap().len(), 3);
    assert_eq!((groups[g["id"].as_str().unwrap()]["order"].as_i64(), groups[second["id"].as_str().unwrap()]["order"].as_i64()), (Some(1), Some(2)));
    assert!(err(&*h, "add_group", json!({ "name": "  " })).contains("required"));
    assert!(err(&*h, "add_group", json!({ "name": "x", "color": "rainbow" })).contains("#RRGGBB"));
    // and a task goes into it by name
    let id = add(&*h, json!({ "title": "背单词", "start": "2026-11-20", "group": "学习" }));
    assert_eq!(stored(&h, &id)["groupId"], g["id"]);
}

#[test]
fn looking_only() {
    let h = host("read-only");
    put(&h.store, json!({ "id": "a", "title": "一", "start": "2026-11-18", "end": "2026-11-18" }));
    h.write.store(false, Ordering::Relaxed);
    let before = h.store.snapshot();
    for (tool, args) in [
        ("add_tasks", json!({ "tasks": [{ "title": "x", "start": "2026-11-20" }] })),
        ("update_task", json!({ "id": "a", "title": "二" })),
        ("set_task_done", json!({ "id": "a" })),
        ("delete_task", json!({ "id": "a" })),
        ("add_group", json!({ "name": "g" })),
    ] {
        let said = err(&*h, tool, args);
        assert!(said.contains("允许修改日历") && said.contains("AI 助手"), "{tool}: {said}");
    }
    assert_eq!(h.store.snapshot(), before, "nothing changed");
    assert_eq!(call(&*h, "list_tasks", json!({ "from": "2026-11-18", "to": "2026-11-18" })).unwrap()["count"], 1, "looking still works");
    assert_eq!(call(&*h, "get_task", json!({ "id": "a" })).unwrap()["title"], "一");
}

// ---------------------------------------------------------------------------------------
// The endpoint
// ---------------------------------------------------------------------------------------

struct Got {
    status: u16,
    headers: ureq::http::HeaderMap,
    body: String,
}

impl Got {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.body))
    }
}

fn send(method: &str, url: &str, headers: &[(&str, &str)], body: &str) -> Got {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).max_redirects(0).proxy(None).build().into();
    let mut b = ureq::http::Request::builder().method(method).uri(url);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let mut res = agent.run(b.body(body.to_owned()).unwrap()).unwrap();
    let body = res.body_mut().read_to_string().unwrap_or_default();
    Got { status: res.status().as_u16(), headers: res.headers().clone(), body }
}

/// One request on its own connection, written byte for byte: (status, headers, body).
fn raw(addr: SocketAddr, request: &str) -> (u16, String, String) {
    let conn = TcpStream::connect(addr).unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    (&conn).write_all(request.as_bytes()).unwrap();
    read_response(&mut BufReader::new(conn))
}

fn read_response(r: &mut BufReader<TcpStream>) -> (u16, String, String) {
    let mut status = String::new();
    r.read_line(&mut status).unwrap();
    let (mut headers, mut len) = (String::new(), None::<usize>);
    loop {
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        let Some((k, v)) = line.trim_end().split_once(':') else { break };
        if k.eq_ignore_ascii_case("content-length") {
            len = v.trim().parse().ok();
        }
        headers.push_str(&line.to_ascii_lowercase());
    }
    let mut body = vec![0; len.unwrap_or_else(|| panic!("no Content-Length after {status}{headers}"))];
    r.read_exact(&mut body).unwrap();
    (status.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0), headers, String::from_utf8(body).unwrap())
}

fn endpoint(name: &str) -> (Arc<TestHost>, Arc<Mutex<String>>, fluentcal_core::httpd::Running, String) {
    let h = host(name);
    let token = Arc::new(Mutex::new(TOKEN.to_owned()));
    let running = http::start(Arc::new(http::Ctx { host: h.clone(), token: token.clone() }), 0).unwrap();
    let url = format!("http://{}/mcp", running.addr);
    (h, token, running, url)
}

#[test]
fn the_endpoint_lets_in_only_who_has_the_token() {
    let (h, token, running, url) = endpoint("http-auth");
    assert!(running.addr.ip().is_loopback(), "this PC only");
    let bearer = format!("Bearer {TOKEN}");
    let ping = legacy("ping", json!({})).to_string();
    let post = |headers: &[(&str, &str)]| send("POST", &url, headers, &ping);

    let ok = post(&[("Authorization", &bearer), ("Content-Type", "application/json")]);
    assert_eq!((ok.status, ok.json()["result"].clone()), (200, json!({})));
    assert_eq!(ok.headers.get("content-type").unwrap(), "application/json");
    assert_eq!(ok.headers.get("cache-control").unwrap(), "no-store");
    assert_eq!(ok.headers.get("x-content-type-options").unwrap(), "nosniff");
    assert!(ok.headers.get("access-control-allow-origin").is_none(), "no page is invited");
    assert_eq!(post(&[("Authorization", &format!("bearer {TOKEN}"))]).status, 200, "the scheme's case does not matter");

    for refused in [post(&[]), post(&[("Authorization", "Bearer wrong")]), post(&[("Authorization", TOKEN)]), post(&[("Authorization", &format!("Basic {TOKEN}"))]), post(&[("Authorization", "Bearer ")])] {
        assert_eq!(refused.status, 401, "{}", refused.body);
        assert!(refused.headers.get("www-authenticate").unwrap().to_str().unwrap().starts_with("Bearer"));
        assert!(!refused.body.contains(TOKEN));
    }
    // a page from elsewhere is not even told whether its token was right
    assert_eq!(post(&[("Authorization", &bearer), ("Origin", "https://evil.example")]).status, 403);
    assert_eq!(post(&[("Authorization", &bearer), ("Origin", "null")]).status, 403);
    assert_eq!(post(&[("Authorization", &bearer), ("Origin", "http://127.0.0.1.evil.example")]).status, 403);
    assert_eq!(post(&[("Origin", "https://evil.example")]).status, 403);
    assert_eq!(post(&[("Authorization", &bearer), ("Origin", "http://localhost:5173")]).status, 200, "a page of this PC, with the token");
    assert_eq!(post(&[("Authorization", &bearer), ("Origin", "http://127.0.0.1")]).status, 200);
    assert_eq!(post(&[("Authorization", &bearer), ("Origin", "vscode-webview://abc")]).status, 200, "a program, not a page");

    // called by another name (DNS rebinding): no
    let named = |name: &str| raw(running.addr, &format!("POST /mcp HTTP/1.1\r\nHost: {name}\r\nAuthorization: {bearer}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{ping}", ping.len())).0;
    assert_eq!(named("evil.example"), 403);
    assert_eq!(named(&format!("evil.example:{}", running.addr.port())), 403);
    assert_eq!(named("localhost"), 200);
    assert_eq!(named(&format!("LOCALHOST:{}", running.addr.port())), 200);
    assert_eq!(named("[::1]:18766"), 200);

    // only the endpoint, only POST
    let base = format!("http://{}", running.addr);
    assert_eq!(send("POST", &format!("{base}/"), &[("Authorization", &bearer)], &ping).status, 404);
    assert_eq!(send("POST", &format!("{base}/mcp/x"), &[("Authorization", &bearer)], &ping).status, 404);
    assert_eq!(send("POST", &format!("{base}/mcp?x=1"), &[("Authorization", &bearer)], &ping).status, 200);
    for method in ["GET", "DELETE", "PUT", "OPTIONS"] {
        let no = send(method, &url, &[("Authorization", &bearer)], "");
        assert_eq!((no.status, no.headers.get("allow").map(|v| v.to_str().unwrap())), (405, Some("POST")), "{method}");
    }
    assert_eq!(send("GET", &url, &[], "").status, 401, "who are you comes first");

    // a new token: the old one is worth nothing
    *token.lock().unwrap() = "another-token".into();
    assert_eq!(post(&[("Authorization", &bearer)]).status, 401);
    assert_eq!(post(&[("Authorization", "Bearer another-token")]).status, 200);
    *token.lock().unwrap() = String::new();
    assert_eq!(post(&[("Authorization", "Bearer ")]).status, 401, "no token: nobody");
    assert_eq!(post(&[("Authorization", "Bearer")]).status, 401);
    drop(h);
}

#[test]
fn the_endpoint_speaks_both_generations() {
    let (h, _token, running, url) = endpoint("http-proto");
    put(&h.store, json!({ "id": "a", "title": "一", "start": "2026-11-18", "end": "2026-11-18" }));
    let bearer = format!("Bearer {TOKEN}");
    let post = |headers: &[(&str, &str)], body: &Value| {
        let mut all = vec![("Authorization", bearer.as_str()), ("Content-Type", "application/json"), ("Accept", "application/json, text/event-stream")];
        all.extend_from_slice(headers);
        send("POST", &url, &all, &body.to_string())
    };

    // the older way: initialize, the notification after it, then calls; no headers needed
    let init = post(&[], &legacy("initialize", json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "old", "version": "1" } })));
    assert_eq!((init.status, init.json()["result"]["protocolVersion"].as_str().map(str::to_owned)), (200, Some("2025-03-26".to_owned())));
    assert!(init.headers.get("mcp-session-id").is_none(), "no sessions");
    let noted = post(&[], &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    assert_eq!((noted.status, noted.body.as_str()), (202, ""));
    let listed = post(&[("MCP-Protocol-Version", "2025-06-18"), ("Mcp-Session-Id", "whatever")], &legacy("tools/call", json!({ "name": "list_tasks", "arguments": { "from": "2026-11-18", "to": "2026-11-18" } })));
    assert_eq!(listed.status, 200);
    assert!(listed.json()["result"]["content"][0]["text"].as_str().unwrap().contains("\"一\""));
    let unknown = post(&[], &legacy("resources/list", json!({})));
    assert_eq!((unknown.status, unknown.json()["error"]["code"].as_i64()), (200, Some(-32601)));

    // the stateless way: the headers say what the message says
    let discover = modern("server/discover", json!({}));
    let ok = post(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "server/discover")], &discover);
    assert_eq!((ok.status, ok.json()["result"]["resultType"].as_str().map(str::to_owned)), (200, Some("complete".to_owned())));
    let call_list = modern("tools/call", json!({ "name": "list_groups", "arguments": {} }));
    let ok = post(&[("mcp-protocol-version", MODERN), ("MCP-METHOD", "tools/call"), ("Mcp-Name", "list_groups")], &call_list);
    assert_eq!(ok.status, 200, "header names in any case: {}", ok.body);
    let ok = post(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call"), ("Mcp-Name", "=?base64?bGlzdF9ncm91cHM=?=")], &call_list);
    assert_eq!(ok.status, 200, "a name wrapped in Base64: {}", ok.body);

    let mismatch = |headers: &[(&str, &str)], body: &Value| {
        let got = post(headers, body);
        assert_eq!((got.status, got.json()["error"]["code"].as_i64(), got.json()["id"].clone()), (400, Some(-32020), json!(7)), "{}", got.body);
        got.json()["error"]["message"].as_str().unwrap().to_owned()
    };
    assert!(mismatch(&[("Mcp-Method", "server/discover")], &discover).contains("MCP-Protocol-Version"));
    assert!(mismatch(&[("MCP-Protocol-Version", "2025-11-25"), ("Mcp-Method", "server/discover")], &discover).contains("does not match"));
    assert!(mismatch(&[("MCP-Protocol-Version", MODERN)], &discover).contains("Mcp-Method"));
    assert!(mismatch(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/list")], &discover).contains("'tools/list'"));
    assert!(mismatch(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "Server/Discover")], &discover).contains("does not match"), "values are compared as they are");
    assert!(mismatch(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call")], &call_list).contains("Mcp-Name"));
    assert!(mismatch(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call"), ("Mcp-Name", "delete_task")], &call_list).contains("'delete_task'"));
    assert!(mismatch(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call"), ("Mcp-Name", "=?base64?!!!?=")], &call_list).contains("Base64"));

    // the status that goes with each error
    let status = |headers: &[(&str, &str)], body: &Value| {
        let got = post(headers, body);
        (got.status, got.json()["error"]["code"].as_i64().unwrap_or(0))
    };
    assert_eq!(status(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "prompts/list")], &modern("prompts/list", json!({}))), (404, -32601));
    let mut future = modern("tools/list", json!({}));
    future["params"]["_meta"][proto::META_VERSION] = json!("2030-01-01");
    let got = post(&[("MCP-Protocol-Version", "2030-01-01"), ("Mcp-Method", "tools/list")], &future);
    assert_eq!((got.status, got.json()["error"]["code"].as_i64(), got.json()["error"]["data"]["supported"][0].as_str().map(str::to_owned)), (400, Some(-32022), Some(MODERN.to_owned())));
    assert_eq!(status(&[("MCP-Protocol-Version", "2030-01-01")], &legacy("tools/list", json!({}))), (400, -32022), "a version in the header nobody knows");
    assert_eq!(status(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/list")], &legacy("tools/list", json!({}))), (400, -32602), "the header says stateless, the message does not say its version");
    let bad = send("POST", &url, &[("Authorization", &bearer)], "{not json");
    assert_eq!((bad.status, bad.json()["error"]["code"].as_i64(), bad.json()["id"].clone()), (400, Some(-32700), Value::Null));
    assert_eq!(status(&[], &json!([legacy("ping", json!({}))])), (400, -32600));
    let empty = send("POST", &url, &[("Authorization", &bearer)], "");
    assert_eq!((empty.status, empty.json()["error"]["code"].as_i64()), (400, Some(-32700)));

    // a change through the endpoint lands in the store
    let added = post(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call"), ("Mcp-Name", "add_tasks")], &modern("tools/call", json!({ "name": "add_tasks", "arguments": { "tasks": [{ "title": "从端点来", "start": "2026-11-20" }] } })));
    assert_eq!(added.status, 200, "{}", added.body);
    assert!(titles(&h.store).contains(&"从端点来".to_owned()));
    drop(running);
}

#[test]
fn large_and_odd_requests_do_no_harm() {
    let (_h, _token, running, url) = endpoint("http-size");
    let bearer = format!("Bearer {TOKEN}");
    let ping = legacy("ping", json!({})).to_string();
    let alive = || assert_eq!(send("POST", &url, &[("Authorization", &bearer)], &ping).status, 200);

    // Too large: refused before it is read, so before anybody looks at who sent it.
    let big = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\",\"pad\":\"{}\"}}", "x".repeat(http::MAX_BODY));
    assert_eq!(send("POST", &url, &[("Authorization", &bearer)], &big).status, 413);
    assert_eq!(send("POST", &url, &[], &big).status, 413);
    alive();
    let nearly = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\",\"pad\":\"{}\"}}", "x".repeat(http::MAX_BODY - 100));
    assert_eq!(send("POST", &url, &[("Authorization", &bearer)], &nearly).status, 200, "up to 1 MB is read");

    // A length nobody could send (1 TB), with a few bytes behind it. The library used at
    // first set aside as much memory as was announced, and the program ended there.
    for auth in [format!("Authorization: {bearer}\r\n"), String::new()] {
        let (status, headers, _) = raw(running.addr, &format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\n{auth}Content-Length: 1099511627776\r\n\r\n{{\"jsonrpc\""));
        assert_eq!(status, 413);
        assert!(headers.contains("connection: close"), "{headers}");
        alive();
    }

    // one connection, many messages: every answer says where it ends
    let conn = TcpStream::connect(running.addr).unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut r = BufReader::new(conn.try_clone().unwrap());
    for i in 0..5 {
        let body = if i == 2 { json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string() } else { ping.clone() };
        (&conn).write_all(format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: {bearer}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
        let (status, headers, answer) = read_response(&mut r);
        assert!(!headers.contains("transfer-encoding"), "request {i}: chunked framing");
        assert_eq!((status, answer.is_empty()), if i == 2 { (202, true) } else { (200, false) }, "request {i}");
    }
    // a refusal leaves a kept connection usable: the message that came with it was read
    (&conn).write_all(format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{ping}", ping.len()).as_bytes()).unwrap();
    assert_eq!(read_response(&mut r).0, 401);
    (&conn).write_all(format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: {bearer}\r\nContent-Length: {}\r\n\r\n{ping}", ping.len()).as_bytes()).unwrap();
    assert_eq!(read_response(&mut r).0, 200);
    // a message sent in chunks (a client that does not know its length beforehand)
    (&conn).write_all(format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: {bearer}\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{ping}\r\n0\r\n\r\n", ping.len()).as_bytes()).unwrap();
    let (status, _, answer) = read_response(&mut r);
    assert_eq!((status, serde_json::from_str::<Value>(&answer).unwrap()["result"].clone()), (200, json!({})));
    drop((conn, r));

    // stopping frees the port at once
    let port = running.addr.port();
    drop(running);
    let again = http::start(Arc::new(http::Ctx { host: host("http-size-2"), token: Arc::new(Mutex::new(TOKEN.into())) }), port);
    assert!(again.is_ok(), "port still taken: {:?}", again.err());
    assert!(http::start(Arc::new(http::Ctx { host: host("http-size-3"), token: Arc::new(Mutex::new(TOKEN.into())) }), port).is_err_and(|e| e.contains("占用")), "a port in use is said so");
}

/// A call that takes its time (the calendar is busy) keeps nobody else waiting: every
/// connection has a thread of its own.
#[test]
fn a_slow_call_does_not_hold_up_the_others() {
    struct Slow(Arc<TestHost>);
    impl Host for Slow {
        fn doc(&self) -> Doc {
            self.0.doc()
        }
        fn can_write(&self) -> bool {
            true
        }
        fn change(&self, f: Change) -> Result<Value, String> {
            std::thread::sleep(Duration::from_millis(1500));
            self.0.change(f)
        }
    }
    let h = host("http-slow");
    let running = http::start(Arc::new(http::Ctx { host: Arc::new(Slow(h.clone())), token: Arc::new(Mutex::new(TOKEN.into())) }), 0).unwrap();
    let url = format!("http://{}/mcp", running.addr);
    let bearer = format!("Bearer {TOKEN}");
    let slow = {
        let (url, bearer) = (url.clone(), bearer.clone());
        std::thread::spawn(move || send("POST", &url, &[("Authorization", &bearer)], &legacy("tools/call", json!({ "name": "add_group", "arguments": { "name": "慢" } })).to_string()))
    };
    std::thread::sleep(Duration::from_millis(200));
    let started = std::time::Instant::now();
    for _ in 0..3 {
        let quick = send("POST", &url, &[("Authorization", &bearer)], &legacy("tools/call", json!({ "name": "list_groups" })).to_string());
        assert_eq!(quick.status, 200);
    }
    assert!(started.elapsed() < Duration::from_millis(1000), "reading waited for the change: {:?}", started.elapsed());
    let done = slow.join().unwrap();
    assert!(done.json()["result"].get("isError").is_none(), "{}", done.body);
    assert_eq!(h.store.snapshot()["groups"].as_object().map(Map::len), Some(1));
}

// ---------------------------------------------------------------------------------------
// The bridge (`fluentcal.exe --mcp`)
// ---------------------------------------------------------------------------------------

/// Feeds lines to the bridge; the lines it answers with.
fn bridged(lines: &[String], target: impl Fn() -> Option<bridge::Target>) -> Vec<Value> {
    let input = lines.join("\n") + "\n";
    let mut out = Vec::new();
    bridge::run(input.as_bytes(), &mut out, target);
    let text = String::from_utf8(out).unwrap();
    assert!(text.is_empty() || text.ends_with('\n'));
    text.lines().map(|l| serde_json::from_str(l).unwrap_or_else(|_| panic!("not one JSON message per line: {l}"))).collect()
}

#[test]
fn the_bridge_without_a_running_calendar() {
    let lines = vec![
        modern("server/discover", json!({})).to_string(),
        legacy("initialize", json!({ "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "c", "version": "1" } })).to_string(),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
        String::new(),
        "   ".into(),
        legacy("tools/list", json!({})).to_string(),
        legacy("tools/call", json!({ "name": "list_tasks", "arguments": {} })).to_string(),
        modern("tools/call", json!({ "name": "add_tasks", "arguments": { "tasks": [{ "title": "x", "start": "2026-11-20" }] } })).to_string(),
        "{oops".into(),
        legacy("ping", json!({})).to_string(),
    ];
    let out = bridged(&lines, || None);
    assert_eq!(out.len(), 7, "one answer per request, none for the notification and the empty lines: {out:?}");
    assert_eq!(out[0]["result"]["supportedVersions"][0], MODERN);
    assert_eq!(out[1]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(out[2]["result"]["tools"].as_array().unwrap().len(), 11, "the tools are known without the calendar");
    for call in [&out[3], &out[4]] {
        assert_eq!(call["result"]["isError"], true, "{call}");
        let why = call["result"]["content"][0]["text"].as_str().unwrap();
        assert!(why.contains("not running") && why.contains("允许 AI 助手连接"), "{why}");
    }
    assert_eq!(out[4]["result"]["resultType"], "complete");
    assert_eq!((out[5]["error"]["code"].as_i64(), out[5]["id"].clone()), (Some(-32700), Value::Null));
    assert_eq!(out[6]["result"], json!({}));

    // a port nobody listens on is the same as none
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let out = bridged(&[legacy("tools/call", json!({ "name": "list_groups" })).to_string()], || Some(bridge::Target { port: free, token: TOKEN.into() }));
    assert!(out[0]["result"]["content"][0]["text"].as_str().unwrap().contains("not running"));
}

#[test]
fn the_bridge_passes_messages_to_the_running_calendar() {
    let (h, token, running, _url) = endpoint("bridge");
    put(&h.store, json!({ "id": "a", "title": "一", "start": "2026-11-18", "end": "2026-11-18" }));
    let port = running.addr.port();
    let target = || Some(bridge::Target { port, token: TOKEN.into() });
    let lines = vec![
        modern("server/discover", json!({})).to_string(),
        modern("tools/call", json!({ "name": "list_tasks", "arguments": { "from": "2026-11-18", "to": "2026-11-18" } })).to_string(),
        legacy("initialize", json!({ "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "c", "version": "1" } })).to_string(),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
        legacy("tools/call", json!({ "name": "add_tasks", "arguments": { "tasks": [{ "title": "经过桥", "start": "2026-11-21" }] } })).to_string(),
        modern("tools/call", json!({ "name": "no_such_tool", "arguments": {} })).to_string(),
        modern("prompts/list", json!({})).to_string(),
    ];
    let out = bridged(&lines, target);
    assert_eq!(out.len(), 6, "{out:?}");
    assert_eq!(out[0]["result"]["resultType"], "complete");
    assert!(out[1]["result"]["content"][0]["text"].as_str().unwrap().contains("\"一\""), "the calendar's own answer: {}", out[1]);
    assert_eq!(out[2]["result"]["protocolVersion"], "2024-11-05");
    assert!(out[3]["result"].get("isError").is_none(), "{}", out[3]);
    assert!(titles(&h.store).contains(&"经过桥".to_owned()), "written by the calendar that runs");
    assert_eq!(out[4]["error"]["code"], -32602);
    assert_eq!(out[5]["error"]["code"], -32601);

    // a token the calendar does not take (a new one was made): said in the call's answer
    *token.lock().unwrap() = "new-token".into();
    let out = bridged(&[legacy("tools/list", json!({})).to_string(), legacy("tools/call", json!({ "name": "list_groups" })).to_string()], target);
    assert_eq!(out[0]["result"]["tools"].as_array().unwrap().len(), 11);
    assert!(out[1]["result"]["content"][0]["text"].as_str().unwrap().contains("access token"), "{}", out[1]);
    // asked again for every message: the new token works without starting anew
    let out = bridged(&[legacy("tools/call", json!({ "name": "list_groups" })).to_string()], || Some(bridge::Target { port, token: "new-token".into() }));
    assert!(out[0]["result"].get("isError").is_none(), "{}", out[0]);
}

#[test]
fn the_bridge_finds_the_calendar_by_its_note() {
    let dir = std::env::temp_dir().join(format!("fluentcal-mcp-state-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(bridge::read_state(&dir), None);
    bridge::write_state(&dir, 18766).unwrap();
    assert_eq!(bridge::read_state(&dir), Some(18766));
    let note: Value = serde_json::from_slice(&std::fs::read(dir.join("mcp.json")).unwrap()).unwrap();
    assert_eq!(note["pid"], std::process::id());
    assert!(!note.to_string().contains("token"), "the token is not written to disk");
    std::fs::write(dir.join("mcp.json"), "{\"port\": 70000}").unwrap();
    assert_eq!(bridge::read_state(&dir), None);
    std::fs::write(dir.join("mcp.json"), "garbage").unwrap();
    assert_eq!(bridge::read_state(&dir), None);
    bridge::clear_state(&dir);
    assert!(!dir.join("mcp.json").exists());
    bridge::clear_state(&dir); // nothing there: fine
    let _ = std::fs::remove_dir_all(&dir);

    // what goes into the Mcp-Name header
    assert_eq!(bridge::header_value("list_tasks"), "list_tasks");
    assert_eq!(bridge::header_value("Hello, 世界"), "=?base64?SGVsbG8sIOS4lueVjA==?=");
    assert_eq!(bridge::header_value(" padded "), "=?base64?IHBhZGRlZCA=?=");
    assert_eq!(bridge::header_value("=?base64?literal?="), "=?base64?PT9iYXNlNjQ/bGl0ZXJhbD89?=");
}

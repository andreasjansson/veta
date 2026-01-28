//! Veta Cloudflare Worker - HTTP API for the Veta knowledge base.

use serde::{Deserialize, Serialize};
use veta_core::{NoteQuery, UpdateNote, VetaService};
use veta_d1::D1DatabaseWrapper;
use worker::*;

#[derive(Deserialize)]
struct CreateNoteRequest {
    title: String,
    body: String,
    tags: Vec<String>,
    /// References to external resources (source code paths, URLs, documentation links, etc.)
    #[serde(default)]
    references: Vec<String>,
}

#[derive(Deserialize)]
struct UpdateNoteRequest {
    title: Option<String>,
    body: Option<String>,
    tags: Option<Vec<String>>,
    /// References to external resources (source code paths, URLs, documentation links, etc.)
    references: Option<Vec<String>>,
}

#[derive(Serialize)]
struct IdResponse {
    id: i64,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
}

fn json_response<T: Serialize>(data: &T, status: u16) -> Result<Response> {
    let body = serde_json::to_string(data).unwrap();
    let mut response = Response::ok(body)?;
    response
        .headers_mut()
        .set("Content-Type", "application/json")?;
    Ok(response.with_status(status))
}

fn json_error(msg: &str, status: u16) -> Result<Response> {
    json_response(&ErrorResponse { error: msg.to_string() }, status)
}

fn get_service(env: &Env) -> Result<VetaService<D1DatabaseWrapper>> {
    let db = env.d1("VETA_DB")?;
    Ok(VetaService::new(D1DatabaseWrapper::new(db)))
}

fn parse_query_tags(url: &Url) -> Option<Vec<String>> {
    url.query_pairs()
        .find(|(k, _)| k == "tags")
        .map(|(_, v)| {
            v.split(',')
                .map(String::from)
                .filter(|s| !s.is_empty())
                .collect()
        })
}

fn parse_query_limit(url: &Url) -> Option<i64> {
    url.query_pairs()
        .find(|(k, _)| k == "limit")
        .and_then(|(_, v)| v.parse().ok())
}

fn parse_query_string(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.to_string())
}

fn parse_query_bool(url: &Url, key: &str) -> bool {
    url.query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v == "true" || v == "1")
        .unwrap_or(false)
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        // POST /notes - Create note
        .post_async("/notes", |mut req, ctx| async move {
            let service = get_service(&ctx.env)?;

            let body: CreateNoteRequest = match req.json().await {
                Ok(b) => b,
                Err(e) => return json_error(&format!("Invalid JSON: {}", e), 400),
            };

            match service.add_note(body.title, body.body, body.tags, body.references).await {
                Ok(id) => json_response(&IdResponse { id }, 201),
                Err(e) => json_error(&e.to_string(), 400),
            }
        })
        // GET /notes - List notes
        .get_async("/notes", |req, ctx| async move {
            let service = get_service(&ctx.env)?;
            let url = req.url()?;

            let query = NoteQuery {
                tags: parse_query_tags(&url),
                from: parse_query_string(&url, "from"),
                to: parse_query_string(&url, "to"),
                limit: parse_query_limit(&url),
            };

            match service.list_notes(query).await {
                Ok(notes) => json_response(&notes, 200),
                Err(e) => json_error(&e.to_string(), 500),
            }
        })
        // GET /notes/:id - Get single note
        .get_async("/notes/:id", |_, ctx| async move {
            let service = get_service(&ctx.env)?;

            let id: i64 = ctx
                .param("id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            match service.get_note(id).await {
                Ok(Some(note)) => json_response(&note, 200),
                Ok(None) => json_error("Not found", 404),
                Err(e) => json_error(&e.to_string(), 500),
            }
        })
        // PATCH /notes/:id - Update note
        .patch_async("/notes/:id", |mut req, ctx| async move {
            let service = get_service(&ctx.env)?;

            let id: i64 = ctx
                .param("id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            let body: UpdateNoteRequest = match req.json().await {
                Ok(b) => b,
                Err(e) => return json_error(&format!("Invalid JSON: {}", e), 400),
            };

            let update = UpdateNote {
                title: body.title,
                body: body.body,
                tags: body.tags,
                references: body.references,
            };

            match service.update_note(id, update).await {
                Ok(true) => json_response(&OkResponse { ok: true }, 200),
                Ok(false) => json_error("Not found", 404),
                Err(e) => json_error(&e.to_string(), 400),
            }
        })
        // DELETE /notes/:id - Delete note
        .delete_async("/notes/:id", |_, ctx| async move {
            let service = get_service(&ctx.env)?;

            let id: i64 = ctx
                .param("id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            match service.delete_note(id).await {
                Ok(true) => json_response(&OkResponse { ok: true }, 200),
                Ok(false) => json_error("Not found", 404),
                Err(e) => json_error(&e.to_string(), 500),
            }
        })
        // GET /tags - List all tags
        .get_async("/tags", |_, ctx| async move {
            let service = get_service(&ctx.env)?;

            match service.list_tags().await {
                Ok(tags) => json_response(&tags, 200),
                Err(e) => json_error(&e.to_string(), 500),
            }
        })
        // GET /grep - Search notes
        .get_async("/grep", |req, ctx| async move {
            let service = get_service(&ctx.env)?;
            let url = req.url()?;

            let pattern = parse_query_string(&url, "q").unwrap_or_default();
            let tags = parse_query_tags(&url);
            let case_sensitive = parse_query_bool(&url, "case_sensitive");

            match service.grep(&pattern, tags, case_sensitive).await {
                Ok(notes) => json_response(&notes, 200),
                Err(e) => json_error(&e.to_string(), 400),
            }
        })
        // POST /migrate - Run migrations
        .post_async("/migrate", |_, ctx| async move {
            let db = ctx.env.d1("VETA_DB")?;
            let wrapper = D1DatabaseWrapper::new(db);

            match wrapper.run_migrations().await {
                Ok(()) => json_response(&OkResponse { ok: true }, 200),
                Err(e) => json_error(&e.to_string(), 500),
            }
        })
        // Health check
        .get("/", |_, _| Response::ok("Veta API"))
        .run(req, env)
        .await
}

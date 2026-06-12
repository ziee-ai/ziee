// Hub models
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Hub model entry.
///
/// Under v2 the identity envelope is reverse-DNS: the manifest's
/// `name` is `io.github.<contributor>/<slug>` and is the catalog
/// lookup key everywhere. The body shape under Hub v2 Phase 7 is
/// parallel to MCP's `packages[]`: a list of `sources[]` (installable
/// variants), with each source carrying its own `quantizations[]`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubModel {
    /// v2 envelope: reverse-DNS canonical name. Matches the
    /// IndexItem.name in the catalog; used as the lookup key on
    /// every install / manifest endpoint.
    pub name: String,
    pub display_name: String,
    /// v2 envelope: per-entry semver (was a single catalog `hub_version`
    /// in v1). Absent on legacy seed entries; the `/installed` updates
    /// path treats `None` as "no update available".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// v2 envelope: schema URL the manifest claims to conform to.
    /// Informational on the consumer; lets the catalog format evolve.
    #[serde(default, rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    /// v2 envelope: namespaced extras
    /// (`io.modelcontextprotocol.registry/*` preserved from ingested
    /// entries on the MCP side). Free-form for forward compat.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    pub description: Option<String>,

    /// Source repository pointer. Mirrors `HubMCPServer.repository`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<HubRepository>,
    /// Project / model homepage (renamed from v1 `homepage_url`).
    #[serde(
        default,
        rename = "websiteUrl",
        alias = "website_url",
        skip_serializing_if = "Option::is_none"
    )]
    pub website_url: Option<String>,

    /// Installable variants. Required — the publisher always sets at
    /// least one. Parallel to `HubMCPServer.packages`.
    pub sources: Vec<ModelSource>,

    /// Soft, informational dependencies (`{kind, name, versionRange}`).
    /// Surfaced in the FE; NOT auto-installed (mirrors `HubAssistant`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<HubDependency>,

    pub capabilities: Option<ModelCapabilities>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub recommended_parameters: Option<serde_json::Value>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub language_support: Option<Vec<String>>,

    /// Whether the model's SOURCE repository currently has a credential
    /// configured. Computed at response time (never read from the catalog file)
    /// by checking, for each `sources[].environment_variables` entry marked
    /// `is_required + is_secret`, whether the matching `llm_repositories` row
    /// has a credential. `true` if at least one source has its required
    /// secret credential configured. When all required-secret sources are
    /// unconfigured, the UI blocks download and points the user to
    /// Settings → LLM Repositories.
    #[serde(default)]
    pub source_auth_configured: bool,

    /// Array of model IDs downloaded by ANYONE from this hub model (system-wide)
    #[serde(default)]
    pub created_ids: Vec<Uuid>,
}

/// Same shape as the v1 `FileFormat`. Kept exactly as before so older
/// catalog entries + the `llm_model` module's translation layer continue
/// to deserialize.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    GGUF,
    SafeTensors,
    PyTorch,
}

/// Type alias matching the spec's `ModelFileFormat` for the v2
/// `ModelSource.file_format` field.
pub type ModelFileFormat = FileFormat;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
#[derive(Default)]
pub struct ModelCapabilities {
    pub vision: bool,
    pub audio: bool,
    pub tools: bool,
    pub code_interpreter: bool,
    pub chat: bool,
    pub text_embedding: bool,
    pub image_generator: bool,
}

/// One installable source variant under `HubModel.sources`. Parallel to
/// MCP's `McpPackage`: each source pins a registry, identifier, version,
/// and (model-specific) one or more `quantizations[]`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelSource {
    /// `"huggingface"` | `"s3"` | `"url"` | `"local"` etc. Drives URL
    /// derivation in the install handler.
    pub registry_type: String,
    /// Repo path (`huggingface`/`s3`) or absolute URL (`url`). For
    /// `huggingface` this is `owner/repo`.
    pub identifier: String,
    /// Branch / commit / tag pin.
    pub version: String,
    /// Per-source file format. Replaces the v1 model-wide `file_format`.
    pub file_format: ModelFileFormat,
    /// `"mistralrs"` | `"llamacpp"` etc. Informational; the install
    /// path does not currently use this to pick an engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_hint: Option<String>,
    /// Per-source context length (replaces v1 model-wide field).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<i32>,
    /// Env vars the source needs (e.g. `HUGGINGFACE_API_KEY`). Reuses
    /// the same `McpKeyValueInput` struct used by `McpPackage` /
    /// `McpRemote`. The install handler's auth gate iterates this list
    /// to find required+secret entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_variables: Vec<McpKeyValueInput>,
    /// Per-source quantization choices. At least one entry per source
    /// (the publisher always lists the default file as a single
    /// quantization). `is_default: true` marks the install-time default.
    pub quantizations: Vec<ModelQuantization>,
}

/// One quantization choice nested under a `ModelSource`. Captures the
/// download-time selection a user makes in the FE (Q4_K_M vs Q5_K_M
/// vs f16, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelQuantization {
    /// Display name (`"Q4_K_M"`, `"f16"`, `"Q8_0"`).
    pub name: String,
    /// File within the source repo to download (matches v1's
    /// `main_filename` semantics — passed straight through to the
    /// download path).
    pub main_file: String,
    /// Size in GB (informational; surfaced in the FE). Optional —
    /// publishers commonly omit it for non-default quants where the
    /// canonical size is the default's. The FE renders "—" when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_gb: Option<f64>,
    /// Optional sha256 (hex) for integrity verification. Not enforced
    /// today; kept on the wire for forward compat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha256: Option<String>,
    /// Marks the install-time default. Exactly one entry SHOULD set
    /// this; the install handler falls back to `quantizations[0]` if
    /// no entry does.
    #[serde(default)]
    pub is_default: bool,
}

/// One informational dependency on a hub entry. Used on both
/// `HubModel` and `HubAssistant`. NOT auto-installed — the FE shows
/// "Works best with…" chips and routes the user to the relevant hub
/// page if they click through.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HubDependency {
    pub kind: DependencyKind,
    /// Reverse-DNS canonical name of the dependency.
    pub name: String,
    /// Semver range (`"^1.0.0"`, `"*"`, etc.). Free-form on the wire;
    /// not validated by the consumer.
    pub version_range: String,
}

/// `kind` discriminant on `HubDependency`. On the wire the values are
/// `model` / `mcp-server` (kebab-case).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyKind {
    Model,
    McpServer,
}

/// Hub assistant entry.
///
/// Like HubModel, the v2 identity envelope is reverse-DNS via `name`.
/// Under Hub v2 Phase 7 the body drops v1's `recommended_models`,
/// `recommended_mcp_servers`, `use_cases`, `example_prompts`,
/// `popularity_score` in favour of a single `dependencies[]` list.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubAssistant {
    /// v2 envelope: reverse-DNS canonical name (the catalog lookup key).
    pub name: String,
    pub display_name: String,
    /// v2 envelope: per-entry semver. See `HubModel.version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub parameters: serde_json::Value,
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub capabilities_required: Option<Vec<String>>,
    pub author: Option<String>,

    /// Source repository pointer (rare for assistants, but supported).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<HubRepository>,
    /// Project / website link.
    #[serde(
        default,
        rename = "websiteUrl",
        alias = "website_url",
        skip_serializing_if = "Option::is_none"
    )]
    pub website_url: Option<String>,

    /// Soft dependencies (`{kind: model|mcp-server, name, versionRange}`).
    /// Replaces v1's `recommended_models` / `recommended_mcp_servers`.
    /// FE renders as "Works best with" chips; NOT auto-installed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<HubDependency>,

    /// Array of entity IDs created by current user from this hub assistant
    #[serde(default)]
    pub created_ids: Vec<Uuid>,

    /// Array of system-wide TEMPLATE assistant IDs installed from this
    /// hub assistant (created_by IS NULL, is_template = true). Usually
    /// 0-or-1 entries — the backend rejects duplicate template installs
    /// with 409. Used by the hub card to disable the "Use as Template"
    /// button when a template install already exists.
    #[serde(default)]
    pub created_template_ids: Vec<Uuid>,
}

/// Hub MCP server entry — strict official `server.json` shape.
///
/// Mirrors `https://static.modelcontextprotocol.io/schemas/2025-09-29/server.schema.json`
/// verbatim. The v1 flat ziee fields (`command`/`args`/`url`/`headers`/
/// `display_name`/`category`/`required_env` etc.) are gone — every install
/// path drives off `packages[]` / `remotes[]` (the official transports).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubMCPServer {
    /// Required: reverse-DNS canonical name (e.g.
    /// `io.github.modelcontextprotocol/filesystem`). Catalog lookup key.
    pub name: String,
    /// One-line human description from the manifest body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Per-entry semver matching the IndexItem.version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Source repository pointer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<HubRepository>,
    /// Project homepage (`websiteUrl` on the wire).
    #[serde(
        default,
        rename = "websiteUrl",
        skip_serializing_if = "Option::is_none"
    )]
    pub website_url: Option<String>,
    /// v2 envelope: schema URL the manifest claims to conform to.
    #[serde(default, rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    /// v2 envelope: namespaced extras. Preserves
    /// `io.modelcontextprotocol.registry/*` keys from ingested entries
    /// so the frontend can surface "official MCP registry" provenance
    /// without a separate lookup.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,

    /// Official `server.json`: stdio-runnable packages
    /// (npm/pypi/oci/nuget/mcpb via npx/uvx/docker/dnx). Ziee filters
    /// to npx/uvx + stdio at install time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<McpPackage>>,

    /// Official `server.json`: remote transports
    /// (streamable-http + sse). Preferred over `packages` if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remotes: Option<Vec<McpRemote>>,

    // ---- Per-response (not on the wire) ----
    /// Array of entity IDs created by current user from this hub server.
    /// Populated by the handler at response time; never present in the
    /// raw manifest JSON.
    #[serde(default)]
    pub created_ids: Vec<Uuid>,

    /// Array of system-wide MCP server IDs installed from this hub
    /// server (created_by IS NULL, is_system = true). Usually 0-or-1
    /// entries — the backend rejects duplicate system installs with
    /// 409. Used by the hub card to disable the "Install as System"
    /// button when a system install already exists. Mirrors
    /// `HubAssistant.created_template_ids`.
    #[serde(default)]
    pub created_system_ids: Vec<Uuid>,
}

// =====================================================
// `server.json` SUB-STRUCTS (the official MCP registry shape).
//
// All field names are camelCase on the wire — that's the shape the
// official schema uses, so deserializing ingested entries verbatim
// avoids a translation layer. The `JsonSchema` derive picks the rename
// rules up for openapi.
// =====================================================

/// Repository pointer block on a manifest entry. Mirrors the official
/// `server.json` `Repository` object.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HubRepository {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subfolder: Option<String>,
}

/// One installable package entry under `HubMCPServer.packages`.
/// Mirrors the official `server.json` `Package` object.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpPackage {
    /// e.g. `"npm"` / `"pypi"` / `"oci"` / `"nuget"` / `"mcpb"`. Ziee
    /// filters to npm/pypi + npx/uvx at install time; other values are
    /// kept on the wire for forward compat but won't be launched.
    pub registry_type: String,
    /// Package name / identifier in the registry (e.g.
    /// `@modelcontextprotocol/server-filesystem`).
    pub identifier: String,
    /// Package version pin. Required by the official schema; the
    /// install path appends it to the command line as part of the
    /// package spec (e.g. `npx -y <identifier>@<version>`).
    pub version: String,
    /// Transport object (official spec — anyOf StdioTransport /
    /// StreamableHttpTransport / SseTransport, each shaped as
    /// `{ "type": "stdio" | "streamable-http" | "sse" }`). Ziee filters
    /// `packages[]` to stdio at install time; the consumer doesn't read
    /// this value today but keeps the struct around to round-trip the
    /// official shape unchanged.
    pub transport: McpTransport,
    /// `"npx"` / `"uvx"` / `"docker"` / `"dnx"`. The install path uses
    /// this verbatim as the spawned command for the supported ones
    /// (npx/uvx); other values are kept for forward compat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_hint: Option<String>,
    /// Args passed to the runtime (e.g. `["-y"]` for npx).
    /// Prepended to the spawned argv (before `identifier` +
    /// `package_arguments`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_arguments: Vec<McpArgument>,
    /// Args passed to the package itself, after `identifier`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub package_arguments: Vec<McpArgument>,
    /// Env vars the package needs. Become the ziee MCP server's
    /// `environment_variables` map.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_variables: Vec<McpKeyValueInput>,
    /// Optional alternate registry hostname.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_base_url: Option<String>,
    /// Optional sha256 (hex) for OCI / mcpb pinning. Not verified by
    /// the consumer today; kept on the wire for forward compat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha256: Option<String>,
}

/// Transport block on an `McpPackage`. The official `server.json`
/// schema models this as an `anyOf` over three single-field structs
/// (`{type: "stdio"}`, `{type: "streamable-http"}`, `{type: "sse"}`).
/// We flatten that to one struct with a single `type` field — round-
/// trips all three shapes cleanly.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpTransport {
    /// `"stdio"` | `"streamable-http"` | `"sse"`. Consumer doesn't
    /// read this today; ziee install path implies stdio whenever a
    /// `package[]` entry is selected.
    #[serde(rename = "type")]
    pub kind: String,
}

/// One remote transport under `HubMCPServer.remotes`. Mirrors the
/// official `server.json` `Remote` object.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpRemote {
    /// Official spelling: `"streamable-http"` (kebab-case) or `"sse"`.
    /// Mapped to ziee `TransportType::Http` / `TransportType::Sse` at
    /// install.
    #[serde(rename = "type")]
    pub transport_kind: String,
    pub url: String,
    /// HTTP headers (may include `${VAR}` interpolation refs that the
    /// install path templates against `environment_variables`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<McpKeyValueInput>,
}

/// Discriminated argument under `McpPackage.runtime_arguments` /
/// `package_arguments`. The official schema is union-shaped
/// `{ type: "positional" | "named", ... }`; this struct flattens both
/// arms (the `name` field is present only for `"named"` args) so a
/// single shape can deserialize either.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpArgument {
    /// `"positional"` (default if absent) | `"named"`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub argument_type: Option<String>,
    /// Flag name for `"named"` args (e.g. `"--workspace"`). None for
    /// positionals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The argument's literal value, when fixed. For an arg the user
    /// supplies at install time, `default` / `value_hint` may be the
    /// only populated fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_repeated: Option<bool>,
    /// One of `"string"` / `"boolean"` / `"number"` / `"filepath"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_hint: Option<String>,
}

/// One env var / header descriptor on an `McpPackage` /
/// `McpRemote`. The official schema names this `KeyValueInput`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpKeyValueInput {
    pub name: String,
    /// Default / suggested value. Mapped into the new MCP
    /// server's env/header map verbatim so the user has a clear
    /// "replace with your token" surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
    #[serde(default)]
    pub is_secret: bool,
}

// =====================================================
// HUB ENTITY TRACKING
// =====================================================

/// Hub entity tracking record
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct HubEntity {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub hub_id: String,
    pub hub_category: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

/// Entity type enum for type safety
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HubEntityType {
    Assistant,
    McpServer,
    LlmModel,
}

impl HubEntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            HubEntityType::Assistant => "assistant",
            HubEntityType::McpServer => "mcp_server",
            HubEntityType::LlmModel => "llm_model",
        }
    }
}

/// Hub category enum. The JSON wire form uses kebab-case (`"mcp-server"`)
/// to match the on-disk folder names in the catalog (`mcp-servers/`) and
/// the index.json shape published by ziee-ai/hub's `release.yml`. The
/// `as_str()` helper still returns the snake-case form (`"mcp_server"`)
/// because the `hub_entities` DB column was created with that value
/// (migration 8's CHECK constraint) — kept for backward compat with
/// existing rows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum HubCategory {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "mcp-server", alias = "mcp_server")]
    McpServer,
    #[serde(rename = "model")]
    Model,
}

impl HubCategory {
    /// Snake-case form for DB rows in `hub_entities.hub_category` —
    /// matches migration 8's CHECK constraint.
    pub fn as_str(&self) -> &'static str {
        match self {
            HubCategory::Assistant => "assistant",
            HubCategory::McpServer => "mcp_server",
            HubCategory::Model => "model",
        }
    }
}

/// Combined hub data structure (for file storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubData {
    pub version: String,
    pub models: Vec<HubModel>,
    pub assistants: Vec<HubAssistant>,
    pub mcp_servers: Vec<HubMCPServer>,
}

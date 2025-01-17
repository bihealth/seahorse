//! Implementation of the Actix server.

pub mod hpo_genes;
pub mod hpo_omims;
pub mod hpo_sim;
pub mod hpo_terms;

use std::{collections::HashMap, sync::Arc};

use actix_web::{middleware::Logger, web::Data, App, HttpServer, ResponseError};
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::OpenApi;

use crate::common::load_hpo;

/// Data structure for the web server data.
pub struct WebServerData {
    /// The HPO ontology (`hpo` crate).
    pub ontology: hpo::Ontology,
    /// Xlink map from NCBI gene ID to HGNC gene ID.
    pub ncbi_to_hgnc: HashMap<u32, String>,
    /// Xlink map from HGNC gene ID to NCBI gene ID.
    pub hgnc_to_ncbi: HashMap<String, u32>,
    /// The full text index over the HPO OBO document.
    pub full_text_index: crate::index::Index,
}

/// Command line arguments for `server run` sub command.
#[derive(clap::Parser, Debug)]
#[command(author, version, about = "Run viguno REST API server", long_about = None)]
pub struct Args {
    /// Path to the directory with the HPO files.
    #[arg(long, required = true)]
    pub path_hpo_dir: String,

    /// Whether to suppress printing hints.
    #[arg(long, default_value_t = false)]
    pub suppress_hints: bool,

    /// IP to listen on.
    #[arg(long, default_value = "127.0.0.1")]
    pub listen_host: String,
    /// Port to listen on.
    #[arg(long, default_value_t = 8080)]
    pub listen_port: u16,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
struct CustomError {
    err: String,
}

impl std::fmt::Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.err)
    }
}

impl CustomError {
    #[allow(clippy::needless_pass_by_value)]
    fn new(err: anyhow::Error) -> Self {
        CustomError {
            err: err.to_string(),
        }
    }
}

impl ResponseError for CustomError {}

/// Specify how to perform query matches in the API calls.
#[derive(Serialize, Deserialize, utoipa::ToSchema, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Match {
    #[default]
    /// Exact string match.
    Exact,
    /// Prefix string match.
    Prefix,
    /// Suffix string match.
    Suffix,
    /// String containment.
    Contains,
}

/// Representation of a gene.
#[derive(
    serde::Deserialize,
    serde::Serialize,
    utoipa::ToSchema,
    Default,
    Debug,
    Clone,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
)]
#[serde_with::skip_serializing_none]
pub struct ResultGene {
    /// The HPO ID.
    pub ncbi_gene_id: u32,
    /// The description.
    pub gene_symbol: String,
    /// The HGNC ID.
    pub hgnc_id: Option<String>,
}

/// Representation of an HPO term.
#[derive(
    serde::Deserialize,
    serde::Serialize,
    utoipa::ToSchema,
    Default,
    Debug,
    Clone,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
)]
pub struct ResultHpoTerm {
    /// The HPO ID.
    pub term_id: String,
    /// The term name.
    pub name: String,
}

/// Helper to deserialize a comma-separated list of strings.
fn vec_str_deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let str_sequence = String::deserialize(deserializer)?;
    Ok(str_sequence
        .split(',')
        .map(std::borrow::ToOwned::to_owned)
        .collect())
}

/// Helper to deserialize a comma-separated list of strings.
fn option_vec_str_deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let str_sequence = String::deserialize(deserializer)?;
    if str_sequence.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            str_sequence
                .split(',')
                .map(std::borrow::ToOwned::to_owned)
                .collect(),
        ))
    }
}

/// Utoipa-based `OpenAPI` generation helper.
#[derive(utoipa::OpenApi)]
#[openapi(
    paths(
        hpo_genes::handle,
        hpo_terms::handle,
        hpo_omims::handle,
        hpo_sim::term_term::handle,
        hpo_sim::term_gene::handle,
    ),
    components(schemas(
        hpo_genes::HpoGenesQuery,
        hpo_genes::HpoGenesResult,
        hpo_genes::HpoGenesResultEntry,
        hpo_omims::HpoOmimsQuery,
        hpo_omims::HpoOmimsResult,
        hpo_omims::HpoOmimsResultEntry,
        hpo_terms::HpoTermsQuery,
        hpo_terms::HpoTermsResult,
        hpo_terms::HpoTermsResultEntry,
        hpo_sim::term_gene::HpoSimTermGeneQuery,
        crate::query::query_result::HpoSimTermGeneResult,
        crate::query::query_result::HpoSimTermGeneResultEntry,
        crate::query::query_result::HpoSimTermGeneTermDetails,
        crate::query::HpoTerm,
        hpo_sim::term_term::HpoSimTermTermQuery,
        hpo_sim::term_term::HpoSimTermTermResult,
        hpo_sim::term_term::HpoSimTermTermResultEntry,
        ResultGene,
        ResultHpoTerm,
        Match,
        crate::common::Version,
        crate::common::IcBasedOn,
        crate::common::SimilarityMethod,
        crate::common::ScoreCombiner,
    ))
)]
pub struct ApiDoc;

/// Main entry point for running the REST server.
#[allow(clippy::unused_async)]
#[actix_web::main]
pub async fn main(args: &Args, dbs: Data<Arc<WebServerData>>) -> std::io::Result<()> {
    let openapi = ApiDoc::openapi();

    HttpServer::new(move || {
        App::new()
            .app_data(dbs.clone())
            .service(hpo_genes::handle)
            .service(hpo_terms::handle)
            .service(hpo_omims::handle)
            .service(hpo_sim::term_term::handle)
            .service(hpo_sim::term_gene::handle)
            .service(
                utoipa_swagger_ui::SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", openapi.clone()),
            )
            .wrap(Logger::default())
    })
    .bind((args.listen_host.as_str(), args.listen_port))?
    .run()
    .await
}

/// Print some hints via `tracing::info!`.
pub fn print_hints(args: &Args) {
    tracing::info!(
        "Launching server main on http://{}:{} ...",
        args.listen_host.as_str(),
        args.listen_port
    );

    // Short-circuit if no hints are to be
    if args.suppress_hints {
        return;
    }

    tracing::info!(
        "  SEE SWAGGER UI FOR INTERACTIVE DOCS: http://{}:{}/swagger-ui/",
        args.listen_host.as_str(),
        args.listen_port
    );
}

/// Main entry point for `run-server` sub command.
///
/// # Errors
///
/// In the case that there is an error running the server.
pub fn run(args_common: &crate::common::Args, args: &Args) -> Result<(), anyhow::Error> {
    tracing::info!("args_common = {:?}", &args_common);
    tracing::info!("args = {:?}", &args);

    if let Some(log::Level::Trace | log::Level::Debug) = args_common.verbose.log_level() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    }

    // Load data that we need for running the server.
    tracing::info!("Loading HPO...");
    let before_loading = std::time::Instant::now();
    let ontology = load_hpo(&args.path_hpo_dir)?;
    tracing::info!("...done loading HPO in {:?}", before_loading.elapsed());

    tracing::info!("Loading HGNC xlink...");
    let before_load_xlink = std::time::Instant::now();
    let path_hgnc_xlink = format!("{}/hgnc_xlink.tsv", args.path_hpo_dir);
    let ncbi_to_hgnc = crate::common::hgnc_xlink::load_ncbi_to_hgnc(path_hgnc_xlink)?;
    let hgnc_to_ncbi = crate::common::hgnc_xlink::inverse_hashmap(&ncbi_to_hgnc);
    tracing::info!(
        "... done loading HGNC xlink in {:?}",
        before_load_xlink.elapsed()
    );

    tracing::info!("Loading HPO OBO...");
    let before_load_obo = std::time::Instant::now();
    let hpo_doc = fastobo::from_file(format!("{}/{}", &args.path_hpo_dir, "hp.obo"))
        .map_err(|e| anyhow::anyhow!("Error loading HPO OBO: {}", e))?;
    tracing::info!(
        "... done loading HPO OBO in {:?}",
        before_load_obo.elapsed()
    );

    tracing::info!("Indexing OBO...");
    let before_index_obo = std::time::Instant::now();
    let full_text_index = crate::index::Index::new(hpo_doc)
        .map_err(|e| anyhow::anyhow!("Error indexing HPO OBO: {}", e))?;
    tracing::info!("... done indexing OBO in {:?}", before_index_obo.elapsed());

    let data = actix_web::web::Data::new(Arc::new(WebServerData {
        ontology,
        ncbi_to_hgnc,
        hgnc_to_ncbi,
        full_text_index,
    }));

    // Print the server URL and some hints (the latter: unless suppressed).
    print_hints(args);
    // Launch the Actix web server.
    main(args, data)?;

    tracing::info!("All done. Have a nice day!");
    Ok(())
}

use axum::Router;

pub mod web;

/// Temporary function to 'build' the service.
/// Will be replaced with a proper builder to set service proprieties.
pub fn make_service() -> Router {
	web::make_service()
}

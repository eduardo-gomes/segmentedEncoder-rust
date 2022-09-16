use axum::Router;

pub fn get_router() -> Router {
	web_packer::include_web_static!()
}

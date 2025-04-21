use ntex::web::HttpResponse;

pub async fn readiness() -> HttpResponse {
    HttpResponse::Ok().finish()
}

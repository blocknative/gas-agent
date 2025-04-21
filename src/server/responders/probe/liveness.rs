use ntex::web::HttpResponse;

pub async fn liveness() -> HttpResponse {
    HttpResponse::Ok().finish()
}

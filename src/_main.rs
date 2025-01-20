use std::ffi::{c_double, c_int};
use actix_web::{App, HttpServer};
use serde::{Deserialize, Serialize};
use utoipa::{ToSchema, OpenApi, IntoParams};
use utoipa_swagger_ui::SwaggerUi;


extern "C" {
    fn optimize_portfolio(
        returns: *const c_double,
        returns_size: c_int,
        cov_matrix: *const c_double,
        cov_matrix_size: c_int,
        risk_free_rate: c_double,
        best_weights: *mut c_double,
    );
}

#[derive(Debug, Serialize, Deserialize, ToSchema, IntoParams)]
struct PortfolioParams {
    returns: Vec<f64>,
    covariance: Vec<Vec<f64>>,
    risk_free_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct WeightResponse {
    weights: Vec<f64>,
}

mod optim {
    use std::ffi::{c_double, c_int};
    use actix_web::{web, HttpResponse, Responder};
    use crate::{optimize_portfolio, PortfolioParams, WeightResponse};

    #[actix_web::get("/optimize")]
    #[utoipa::path(
        get,
        path = "/optimize",
        params(
            PortfolioParams
        ),
        responses(
        (status = 200, description = "Optimized portfolio weights", body = WeightResponse)
        ),
    )]
    pub async fn optimize_portfolio_api(params: web::Query<PortfolioParams>) -> impl Responder {
        let n = params.returns.len() as c_int;
        let m = params.covariance.len() as c_int;

        let mut best_weights: Vec<f64> = vec![0.0; n as usize];

        let flat_cov_matrix: Vec<c_double> = params
            .covariance
            .iter()
            .flat_map(|row| row.iter().cloned())
            .collect();

        unsafe {
            optimize_portfolio(
                params.returns.as_ptr(),
                n,
                flat_cov_matrix.as_ptr(),
                m,
                params.risk_free_rate,
                best_weights.as_mut_ptr(),
            );
        }

        HttpResponse::Ok().json(WeightResponse { weights: best_weights })
    }
}

#[derive(OpenApi)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let openapi = ApiDoc::openapi();

    let address = "127.0.0.1:8080";

    let server_result = HttpServer::new(move || {
        App::new()
            .service(optim::optimize_portfolio_api)
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-doc/openapi.json", openapi.clone())
            )
    })
        .bind(address);

    match server_result {
        Ok(server) => {
            println!("Server started successfully at http://{}", address);
            println!("Swagger UI available at http://{}/swagger-ui/", address);
            server.run().await
        },
        Err(e) => {
            eprintln!("Failed to start server: {}", e);
            Err(e)
        }
    }
}
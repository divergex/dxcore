#include <iostream>
#include <vector>
#include <Eigen/Dense>
#include <omp.h>

void mean_variance_optimization(const std::vector<double>& returns, const std::vector<std::vector<double>>& cov_matrix,
                                const double min_leverage = 0.0, const double max_leverage = 1.0) {
    const int N = static_cast<int>(returns.size());

    if (cov_matrix.size() != N || cov_matrix[0].size() != N) {
        std::cerr << "Error: Covariance matrix must be N x N, where N is the number of assets." << std::endl;
        return;
    }

    Eigen::MatrixXd A(N, N);
    Eigen::VectorXd B(N);
    Eigen::VectorXd weights(N);

    #pragma omp parallel for default(none) shared(A, B, returns, cov_matrix) num_threads(4)
    for (int i = 0; i < N; ++i) {
        for (int j = 0; j < N; ++j) {
            A(i, j) = cov_matrix[i][j];
        }
    }

    #pragma omp parallel for default(none) shared(B, returns) num_threads(4)
    for (int i = 0; i < N; ++i) {
        B(i) = returns[i];
    }

    double epsilon = 1e-5; // Regularization parameter
    Eigen::MatrixXd regularized_A = A + epsilon * Eigen::MatrixXd::Identity(N, N);
    Eigen::MatrixXd cov_matrix_inv = regularized_A.inverse();

    weights = cov_matrix_inv * B;

    double sum_weights = weights.sum();
    if (sum_weights > 0) {
        weights /= sum_weights;
    }

    #pragma omp parallel for default(none) shared(weights, min_leverage, max_leverage) num_threads(4)
    for (int i = 0; i < N; ++i) {
        if (weights(i) < min_leverage) {
            weights(i) = min_leverage;
        } else if (weights(i) > max_leverage) {
            weights(i) = max_leverage;
        }
    }

    sum_weights = weights.sum();
    if (sum_weights > 0) {
        weights /= sum_weights;
    }

    std::cout << "Optimized weights (with constraints): ";
    for (int i = 0; i < N; ++i) {
        std::cout << weights(i) << " ";
    }
    std::cout << std::endl;
}

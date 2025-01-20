#include <Eigen/Dense>
#include <iostream>
#include <vector>

using namespace Eigen;

extern "C" {
    void optimize_portfolio(const double* returns, int returns_size, const double* cov_matrix, int cov_matrix_size, double risk_free_rate, double* best_weights) {
        const int n = returns_size;

        VectorXd returns_vec = Map<const VectorXd>(returns, n);
        MatrixXd cov_matrix_mat = Map<const MatrixXd>(cov_matrix, n, n);

        VectorXd excess_returns = returns_vec - risk_free_rate * VectorXd::Ones(n);

        VectorXd weights = VectorXd::Constant(n, 1.0 / n);

        std::vector<int> active_set(n, 1);  // 1 if active, 0 otherwise
        bool optimized = false;

        while (!optimized) {
            VectorXd gradient = cov_matrix_mat * weights - excess_returns;

            optimized = true;
            for (int i = 0; i < n; ++i) {
                if (weights(i) < 0) {
                    active_set[i] = 0;
                    weights(i) = 0;
                    optimized = false;
                }
            }

            std::vector<int> active_indices;
            for (int i = 0; i < n; ++i) {
                if (active_set[i]) active_indices.push_back(i);
            }

            int active_count = active_indices.size();
            if (active_count == 0) break;

            MatrixXd cov_active(active_count, active_count);
            VectorXd excess_active(active_count);

            for (int i = 0; i < active_count; ++i) {
                excess_active(i) = excess_returns(active_indices[i]);
                for (int j = 0; j < active_count; ++j) {
                    cov_active(i, j) = cov_matrix_mat(active_indices[i], active_indices[j]);
                }
            }

            VectorXd weights_active = cov_active.ldlt().solve(excess_active);

            for (int i = 0; i < active_count; ++i) {
                int index = active_indices[i];
                weights(index) = std::max(0.0, weights_active(i));
            }
        }

        weights /= weights.sum();

        Map<VectorXd>(best_weights, n) = weights;
    }
}

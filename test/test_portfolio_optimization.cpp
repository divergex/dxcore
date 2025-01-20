#include <algorithm>
#include <cmath>
#include <numeric>
#include <vector>
#include <gtest/gtest.h>

extern "C" {
    void optimize_portfolio(const double* returns, int returns_size, const double* cov_matrix, int cov_matrix_size, double risk_free_rate, double* best_weights);
}

TEST(OptimizePortfolioTest, BasicTest) {
    const std::vector returns = {0.1, 0.2, 0.15};
    const std::vector cov_matrix = {0.04, 0.01, 0.02, 0.01, 0.03, 0.015, 0.02, 0.015, 0.05};
    constexpr double risk_free_rate = 0.05;

    const int n = returns.size();
    std::vector best_weights(n, 0.0);

    optimize_portfolio(returns.data(), n, cov_matrix.data(), n, risk_free_rate, best_weights.data());

    const bool all_zero = std::ranges::all_of(best_weights, [](double w) { return w == 0.0; });
    ASSERT_FALSE(all_zero) << "Portfolio weights should not be all zero";

    const double sum_weights = std::accumulate(best_weights.begin(), best_weights.end(), 0.0);
    EXPECT_NEAR(sum_weights, 1.0, 0.01) << "Sum of portfolio weights should be close to 1";


    // Log final weights
    std::cout << "Final portfolio weights: ";  // Expected: 0.5 0.5 0.0
    for (const auto& w : best_weights) {
        std::cout << w << " ";
    }
}

//
// Created by rzimmerdev on 11/5/24.
//

#ifndef PORTFOLIO_OPTIMIZATION_H
#define PORTFOLIO_OPTIMIZATION_H

void optimize_portfolio(const double* returns, int returns_size, const double* cov_matrix, int cov_matrix_size, double risk_free_rate, double* best_weights);

#endif //PORTFOLIO_OPTIMIZATION_H

//
// Created by rzimmerdev on 10/15/24.
//

#ifndef MVO_H
#define MVO_H

void mean_variance_optimization(const std::vector<double> &returns, const std::vector<std::vector<double> > &cov_matrix,
                                double min_leverage = 0.0, double max_leverage = 1.0);

#endif //MVO_H

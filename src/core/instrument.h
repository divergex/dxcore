// instrument.h
#ifndef INSTRUMENT_H
#define INSTRUMENT_H

#include <string>
#include <stdexcept>

class Instrument {
public:
    Instrument() = default;
    explicit Instrument(const std::string& symbol): symbol(symbol)
    {
        if (symbol.empty())
            throw std::invalid_argument("symbol cannot be empty");
    }

    const std::string symbol;
    const std::string exchange;
};

#endif // INSTRUMENT_H

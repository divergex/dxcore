#ifndef ORDER_BOOK_H
#define ORDER_BOOK_H

#include "order.h"
#include "price_level.h"
#include "transaction.h"
#include "instrument.h"
#include <map>
#include <vector>
#include <boost/uuid/uuid_hash.hpp>

/**
 * @brief Central limit order book for a single instrument.
 *
 * Maintains a price-time priority queue of resting orders across
 * bid and ask sides. Incoming aggressive orders are matched greedily
 * against the best available levels, generating @ref Transaction records.
 *
 * Prices are snapped to the nearest tick on insertion. Empty levels
 * are pruned lazily to avoid unbounded map growth.
 */
class OrderBook {
public:
    /**
     * @brief Constructs an empty order book for the given instrument.
     * @param instrument  The instrument this book tracks.
     * @param tickSize    Minimum price increment; defaults to 0.01.
     */
    explicit OrderBook(Instrument instrument, double tickSize = 0.01);

    /**
     * @brief Inserts an order, triggering matching if it is aggressive.
     *
     * If the order crosses the spread it will match against resting
     * liquidity in price-time priority. Any unfilled remainder is posted
     * as a passive resting order.
     *
     * @param order  Order to insert (taken by value; may be mutated).
     * @return       Vector of transactions produced by matching; empty if
     *               the order rests without matching.
     */
    std::vector<Transaction> insert(Order order);

    /**
     * @brief Cancels a resting order by ID.
     * @param id  Identifier of the order to cancel.
     * @note No-op if the ID is not found in the book.
     */
    void cancel(const Order::Id& id);

    /**
     * @brief Returns the mid-price, i.e. (bestBid + bestAsk) / 2.
     * @return Mid-price, or NaN if either side is empty.
     */
    double getMidPrice() const;

    /**
     * @brief Returns the best (highest) bid price.
     * @return Best bid, or NaN if the bid side is empty.
     */
    double getBestBid() const;

    /**
     * @brief Returns the best (lowest) ask price.
     * @return Best ask, or NaN if the ask side is empty.
     */
    double getBestAsk() const;

    /**
     * @brief Returns the total number of non-empty price levels across both sides.
     */
    size_t getLevelCount() const;

    /**
     * @brief Returns the instrument associated with this book.
     */
    const Instrument& getInstrument() const { return instrument; }

private:
    /// Bid side: descending map so best bid is always at `begin()`.
    using Bids = std::map<double, PriceLevel, std::greater<>>;

    /// Ask side: ascending map so best ask is always at `begin()`.
    using Asks = std::map<double, PriceLevel>;

    /// Maps order ID → price for O(1) level lookup on cancel.
    using OrderIndex = std::unordered_map<Order::Id, double>;

    /**
     * @brief Rounds @p price to the nearest tick boundary.
     */
    double snapToTick(double price) const;

    /**
     * @brief Removes price levels within @p threshold ticks of @p midPrice
     *        that have become empty after matching or cancellation.
     */
    void pruneEmptyLevels(double midPrice, double threshold);

    /**
     * @brief Matches a buy aggressor against resting asks.
     * @param aggressor  Incoming buy order; its remaining quantity is
     *                   decremented in place as it fills.
     * @return           Transactions generated during matching.
     */
    std::vector<Transaction> matchAgainstAsks(Order& aggressor);

    /**
     * @brief Matches a sell aggressor against resting bids.
     * @param aggressor  Incoming sell order; mutated as above.
     * @return           Transactions generated during matching.
     */
    std::vector<Transaction> matchAgainstBids(Order& aggressor);

    Instrument   instrument;
    double       tickSize;
    Bids         bids;
    Asks         asks;
    OrderIndex   orderIndex;
};

#endif // ORDER_BOOK_H
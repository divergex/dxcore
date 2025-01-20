#include <zmq.hpp>
#include <boost/asio.hpp>
#include <iostream>
#include <string>
#include <map>
#include <vector>

using namespace boost::asio;

class MarketDataProcessor {
public:
    MarketDataProcessor(io_service &ios) : acceptor_(ios, ip::tcp::endpoint(ip::tcp::v4(), 12345)) {
        std::cout << "Listening on TCP port 12345" << std::endl;
    }

    void startListening() {
        startAccepting();
        io_service.run();
    }

    // Start listening to multiple exchange interfaces (ZeroMQ channels)
    void startListeningToInterfaces(const std::vector<std::string> &channels) {
        for (const auto &channel : channels) {
            zmq::context_t context(1);
            zmq::socket_t socket(context, ZMQ_PULL);
            socket.connect(channel); // Connect to each exchange interface

            // Listen for market data
            std::thread listener_thread([this, socket]() {
                while (true) {
                    zmq::message_t message;
                    socket.recv(message);
                    std::string market_data(static_cast<char*>(message.data()), message.size());
                    processMarketData(market_data);
                }
            });
            listener_threads.push_back(std::move(listener_thread));
        }
    }

private:
    ip::tcp::acceptor acceptor_;
    std::map<int, std::string> order_book_; // Example limit order book representation
    std::vector<std::thread> listener_threads; // For listening to multiple interfaces

    void startAccepting() {
        ip::tcp::socket socket(acceptor_.get_io_service());
        acceptor_.async_accept(socket, [this, &socket](const boost::system::error_code &ec) {
            if (!ec) {
                handleConnection(std::move(socket));
            }
            startAccepting();
        });
    }

    void handleConnection(ip::tcp::socket socket) {
        // Handle incoming data from client
        std::string message = "Market Data Aggregated Feed";  // Replace with actual processed data
        boost::asio::async_write(socket, buffer(message),
            [this](boost::system::error_code ec, std::size_t bytes_transferred) {
                if (!ec) {
                    std::cout << "Sent market data to client." << std::endl;
                } else {
                    std::cout << "Error in sending data: " << ec.message() << std::endl;
                }
            });
    }

    void processMarketData(const std::string &data) {
        // Example market data processing logic
        std::cout << "Processing Market Data: " << data << std::endl;
    }
};

int main() {
    io_service ios;
    MarketDataProcessor processor(ios);

    // Start listening to multiple interfaces (exchange connections)
    std::vector<std::string> exchange_interfaces = {"tcp://localhost:5555", "tcp://localhost:5556"};
    processor.startListeningToInterfaces(exchange_interfaces);

    // Start the market data processor accepting client connections
    std::thread processor_thread([&processor]() {
        processor.startListening();
    });

    processor_thread.join();
    return 0;
}

#include <CL/sycl.hpp>
#include <zmq.hpp>
#include <quickfix/quickfix.h>
#include <iostream>

using namespace sycl;

class MarketDataReceiver {
public:
    void operator()(queue &q) {
        // Initialize ZMQ context and socket
        zmq::context_t context(1);
        zmq::socket_t socket(context, ZMQ_PUSH);
        socket.connect("tcp://localhost:5555"); // Connect to a predefined channel (e.g., centralized unit)

        // QuickFIX Message Parser and Handler
        FIX::SessionSettings settings("quickfix.cfg");
        FIX::FileStoreFactory storeFactory(settings);
        FIX::ThreadedSocketInitiator initiator(storeFactory, settings, *this);
        initiator.start();

        // FPGA Data Processing Placeholder
        q.submit([&](handler &h) {
            h.single_task([&]() {
                // Simulate market data fetching and send it to ZeroMQ channel
                std::string market_data = "Sample Market Data"; // Replace with actual data
                socket.send(zmq::buffer(market_data), zmq::send_flags::none);
            });
        }).wait();
    }

    void onMessage(const FIX::Message& message) {
        // Parse QuickFIX message and prepare data for ZeroMQ
        std::string market_data = message.toString(); // Process market data here
        std::cout << "Received Market Data: " << market_data << std::endl;
        // Further processing can be done before sending via ZeroMQ
    }
};

int main() {
    // DPC++ queue for FPGA or CPU/GPU execution
    queue q{cpu_selector{}};

    MarketDataReceiver receiver;
    receiver(q);

    return 0;
}

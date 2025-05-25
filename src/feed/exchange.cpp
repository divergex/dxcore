#include <cstring>
#include <iostream>
#include <sycl/sycl.hpp>
#include <zmq.hpp>

using namespace sycl;

void generate_data(char *data, size_t max_size) {
    const char sample[] = "Sample Market Data from device";
    size_t len = sizeof(sample);
    if (len > max_size) len = max_size;
    memcpy(data, sample, len);
}

class MarketDataReceiver {
   public:
    void operator()(queue &q) {
        std::cout << "Start operator()\n";
        try {
            zmq::context_t context(1);
            zmq::socket_t socket(context, ZMQ_PUSH);

            // 🔧 Prevent blocking behavior
            socket.set(zmq::sockopt::linger, 0);
            socket.set(zmq::sockopt::sndhwm, 1);
            socket.set(zmq::sockopt::sndtimeo, 100);  // in milliseconds

            socket.connect("tcp://localhost:5555");
            std::cout << "ZMQ socket connected\n";

            constexpr size_t data_size = 64;
            char data_buffer[data_size] = {0};
            {
                buffer<char, 1> buf(data_buffer, range<1>(data_size));
                std::cout << "Buffer created\n";

                auto e = q.submit([&](handler &h) {
                    auto acc = buf.get_access<access::mode::write>(h);
                    h.single_task([=]() {
                        const char sample[] = "Sample Market Data from device";
                        size_t len = sizeof(sample);
                        if (len > data_size) len = data_size;
                        for (size_t i = 0; i < len; i++) {
                            acc[i] = sample[i];
                        }
                    });
                });
                std::cout << "Kernel submitted\n";

                try {
                    e.wait_and_throw();  // will rethrow any SYCL errors
                } catch (sycl::exception const &e) {
                    std::cerr << "SYCL exception: " << e.what() << "\n";
                    std::exit(1);
                }
                std::cout << "Kernel finished\n";
            }  // buffer syncs back here

            std::string market_data(data_buffer);
            std::cout << "Market data ready: " << market_data << "\n";

            auto send_result = socket.send(zmq::buffer(market_data),
                                           zmq::send_flags::dontwait);
            if (send_result.has_value()) {
                std::cout << "Message sent successfully, bytes: "
                          << *send_result << "\n";
            } else {
                std::cerr << "Message not sent (receiver not connected?)\n";
            }

        } catch (const zmq::error_t &err) {
            std::cerr << "ZMQ Error: " << err.what() << "\n";
        }

        std::cout << "End operator()\n";
    }
};

int main() {
    queue q{cpu_selector_v};

    MarketDataReceiver receiver;
    receiver(q);

    std::cout << "Finished receiver call, exiting main.\n";
    return 0;
}

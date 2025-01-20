import socket

def fetch_market_data():
    # Connect to the market data server on TCP port 12345
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.connect(('localhost', 12345))
        s.sendall(b"Requesting Market Data")  # Optional message
        data = s.recv(1024)
    print('Received:', data.decode())

if __name__ == "__main__":
    fetch_market_data()

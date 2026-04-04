set(PYBIND11_FINDPYTHON ON)
find_package(pybind11 CONFIG REQUIRED)

pybind11_add_module(market market.cpp)  # Change 'python' to 'market'
target_link_libraries(market PRIVATE dxcore)

install(TARGETS market DESTINATION dxcore)
install(FILES ${PROJECT_SOURCE_DIR}/dxcore/__init__.py
        ${PROJECT_SOURCE_DIR}/dxcore/market.pyi
        DESTINATION dxcore)
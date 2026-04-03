find_package(Python COMPONENTS Interpreter Development REQUIRED)
find_package(pybind11 REQUIRED)

pybind11_add_module(market market.cpp)
target_link_libraries(market PRIVATE dxcore)

install(TARGETS market DESTINATION dxcore)
install(FILES ${PROJECT_SOURCE_DIR}/dxcore/__init__.py
        ${PROJECT_SOURCE_DIR}/dxcore/market.pyi
        DESTINATION dxcore)

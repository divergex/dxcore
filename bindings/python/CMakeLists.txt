find_package(Python COMPONENTS Interpreter Development REQUIRED)
find_package(pybind11 REQUIRED)

pybind11_add_module(python market.cpp)
target_link_libraries(python PRIVATE dxcore)

install(TARGETS python DESTINATION dxcore)
install(FILES ${PROJECT_SOURCE_DIR}/dxcore/__init__.py
        ${PROJECT_SOURCE_DIR}/dxcore/market.pyi
        DESTINATION dxcore)

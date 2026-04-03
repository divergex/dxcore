from conan import ConanFile
from conan.tools.cmake import cmake_layout, CMakeToolchain, CMakeDeps

class DxcoreConan(ConanFile):
    name = "dxcore"
    version = "0.1.0"
    package_type = "application"
    settings = "os", "compiler", "build_type", "arch"

    # Optional: declare options if you need them
    options = {
        "shared": [True, False],
        "fPIC": [True, False],
    }
    default_options = {
        "shared": False,
        "fPIC": True,
    }

    def layout(self):
        # Use cmake_layout to organize the build properly
        # This puts generators in the build folder root for easier access
        cmake_layout(self)

    def generate(self):
        # Generate the toolchain - this creates conan_toolchain.cmake
        tc = CMakeToolchain(self)
        tc.user_presets_path = False  # Don't create CMakeUserPresets.json
        tc.generate()

        # Generate dependency configs - this creates FindBoost.cmake, etc.
        deps = CMakeDeps(self)
        deps.generate()

    def requirements(self):
        if hasattr(self, 'conan_data') and self.conan_data:
            for requirement in self.conan_data.get('requirements', []):
                self.requires(requirement)
            for requirement in self.conan_data.get('test_requires', []):
                self.test_requires(requirement)
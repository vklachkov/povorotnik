cmake_minimum_required(VERSION 3.12)

set(PICO_BOARD "pico_w")
include("$ENV{PICO_SDK_PATH}/pico_sdk_init.cmake")

project("povorotnik" C CXX ASM)
set(CMAKE_TRY_COMPILE_TARGET_TYPE "STATIC_LIBRARY")
set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

add_compile_options("-Wall" "-Wno-format" "-Wno-unused-function")
if (CMAKE_C_COMPILER_ID STREQUAL "GNU")
    add_compile_options("-Wno-maybe-uninitialized")
endif()

pico_sdk_init()

add_executable("povorotnik"
    "src/main.c"
)

target_include_directories("povorotnik" PRIVATE "${CMAKE_CURRENT_LIST_DIR}/src")  # for btstack config
pico_btstack_make_gatt_header("povorotnik" PRIVATE "${CMAKE_CURRENT_LIST_DIR}/src/profile.gatt")

pico_enable_stdio_usb("povorotnik" 1)

target_link_libraries("povorotnik"
    "pico_stdlib"
    "pico_cyw43_arch_none"
    "pico_btstack_ble"
    "pico_btstack_cyw43"
    "hardware_i2c"
)

pico_add_extra_outputs("povorotnik")
cmake_minimum_required(VERSION 3.16)

set(PICO_BOARD pico2 CACHE STRING "Pico board type")

if(NOT DEFINED ENV{PICO_SDK_PATH})
    message(FATAL_ERROR "Set PICO_SDK_PATH to your pico-sdk checkout before configuring.")
endif()

include($ENV{PICO_SDK_PATH}/external/pico_sdk_import.cmake)

project(nes_sound C CXX ASM)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

pico_sdk_init()

add_executable(nes_sound
    src/main.c
    src/nes_apu.c
)

target_include_directories(nes_sound PRIVATE
    ${CMAKE_CURRENT_LIST_DIR}/src
)

target_link_libraries(nes_sound PRIVATE
    pico_stdlib
    hardware_clocks
    hardware_pwm
)

pico_enable_stdio_usb(nes_sound 0)
pico_enable_stdio_uart(nes_sound 1)

pico_add_extra_outputs(nes_sound)

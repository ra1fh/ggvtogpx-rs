
cmake_minimum_required(VERSION 3.16)

project(ggvtogpx VERSION 1.0.0 LANGUAGES NONE)
include(CTest)

find_program(CARGO_EXECUTABLE cargo REQUIRED)

set(BINARY_NAME "ggvtogpx")
set(RUST_OUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/target")
if(WIN32)
  set(BIN_PATH "${RUST_OUT_DIR}/${BINARY_NAME}.exe")
else()
  set(BIN_PATH "${RUST_OUT_DIR}/release/${BINARY_NAME}")
endif()

add_custom_target(ggvtogpx ALL
  COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${RUST_OUT_DIR}
          ${CARGO_EXECUTABLE} build --release
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})

set_property(DIRECTORY PROPERTY ADDITIONAL_MAKE_CLEAN_FILES ${BIN_PATH})
install(PROGRAMS ${BIN_PATH} DESTINATION bin)

set (BinTestsToRun
  ggv_bin-sample-v2
  ggv_bin-sample-v3
  ggv_bin-sample-v4
  ggv_ovl-sample-1
  ggv_ovl-sample-2
  ggv_xml-sample-1
  ggv_xml-sample-2
  ggv_xml-sample-3
  ggv_xml-sample-4
  ggv_xml-sample-5)

add_test(NAME build_ggvtogpx_for_tests
  COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${RUST_OUT_DIR}
          ${CARGO_EXECUTABLE} build --release
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)

# Add all the ADD_TEST for each test
foreach (test ${BinTestsToRun})
  add_test (NAME ${test}-generate COMMAND ${BIN_PATH} ${CMAKE_SOURCE_DIR}/testdata/${test}.ovl ${test}.out)
  add_test (NAME ${test}-diff COMMAND ${CMAKE_COMMAND} -E compare_files ${CMAKE_SOURCE_DIR}/testdata/${test}.gpx ${test}.out)
  add_test (NAME ${test}-loopback-generate COMMAND ${BIN_PATH} -i gpx ${CMAKE_SOURCE_DIR}/testdata/${test}.gpx ${test}-loopback.out)
  add_test (NAME ${test}-loopback-diff COMMAND ${CMAKE_COMMAND} -E compare_files ${CMAKE_SOURCE_DIR}/testdata/${test}.gpx ${test}-loopback.out)
  set_tests_properties(${test}-generate PROPERTIES ENVIRONMENT "GGVTOGPX_TESTMODE=1")
  set_tests_properties(${test}-loopback-generate PROPERTIES ENVIRONMENT "GGVTOGPX_TESTMODE=1")
  set_tests_properties(build_ggvtogpx_for_tests PROPERTIES FIXTURES_SETUP rust_build_fixture)
set_tests_properties(${test}-generate PROPERTIES FIXTURES_REQUIRED rust_build_fixture)
endforeach ()

add_custom_target(diff)
foreach(test ${BinTestsToRun})
add_custom_command(TARGET diff POST_BUILD
  COMMAND diff -u ${CMAKE_SOURCE_DIR}/testdata/${test}.gpx ${test}.out)
add_custom_command(TARGET diff POST_BUILD
  COMMAND diff -u ${CMAKE_SOURCE_DIR}/testdata/${test}.gpx ${test}-loopback.out)
endforeach()

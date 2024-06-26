# Trying to learn Rust...

This implements the TestRunner in Rust, see: http://github.com/gnilk/testrunner

I wanted a project to test out Rust. The test-runner is fairly contained and contains
various interesting aspects (threads, interfacing with dynamically loaded libraries).

Plus, I can use the real version as a 'unit-test'.

## TO-DO List
<pre>
! Dependency handling
! Pre/Post case handling
- Better internal return codes (in quite a lot of places)
! Threading for test-case execution
  - Ability to switch of threading from cmd-line..
+ Output formatting
! Nicer split in sub-modules
- Reporting
! Separate TestResult structure
  ! Handle assert-errors and store in TestResult
! Rename 'TestResultClass' to TestReturnCode
! Circular dependencies ('cdepends' from unit-test of testrunner)
! Refactor the way modules and cases are handled, look at 'testrunner.cpp' from the original testrunner project
- Update Pre/Post callbacks to V2 (i.e. returning int's).
- Add support for cmd-line specification of test/module using pattern matching
</pre>

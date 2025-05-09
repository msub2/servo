/*
Copyright (c) 2019 The Khronos Group Inc.
Use of this source code is governed by an MIT-style license that can be
found in the LICENSE.txt file.
*/

(function() {
  var testHarnessInitialized = false;

  var initNonKhronosFramework = function() {
    if (testHarnessInitialized) {
      return;
    }
    testHarnessInitialized = true;

    /* -- plaform specific code -- */

    // WebKit Specific code. Add your code here.
    if (window.testRunner && !window.layoutTestController) {
      window.layoutTestController = window.testRunner;
    }

    if (window.layoutTestController) {
      window.layoutTestController.dumpAsText();
      window.layoutTestController.waitUntilDone();
    }
    if (window.internals) {
      // The WebKit testing system compares console output.
      // Because the output of the WebGL Tests is GPU dependent
      // we turn off console messages.
      window.console.log = function() { };
      window.console.error = function() { };
      window.internals.settings.setWebGLErrorsToConsoleEnabled(false);
    }

    /* -- end platform specific code --*/
  }

  this.initTestingHarness = function() {
    initNonKhronosFramework();
  }
}());

var getUrlOptions = (function() {
  var _urlOptionsParsed = false;
  var _urlOptions = {};
  return function() {
    if (!_urlOptionsParsed) {
      var s = window.location.href;
      var q = s.indexOf("?");
      var e = s.indexOf("#");
      if (e < 0) {
        e = s.length;
      }
      var query = s.substring(q + 1, e);
      var pairs = query.split("&");
      for (var ii = 0; ii < pairs.length; ++ii) {
        var keyValue = pairs[ii].split("=");
        var key = keyValue[0];
        var value = decodeURIComponent(keyValue[1]);
        _urlOptions[key] = value;
      }
      _urlOptionsParsed = true;
    }

    return _urlOptions;
  }
})();

if (typeof quietMode == 'undefined') {
  var quietMode = (function() {
    var _quietModeChecked = false;
    var _isQuiet = false;
    return function() {
      if (!_quietModeChecked) {
        _isQuiet = (getUrlOptions().quiet == 1);
        _quietModeChecked = true;
      }
      return _isQuiet;
    }
  })();
}

function nonKhronosFrameworkNotifyDone() {
  // WebKit Specific code. Add your code here.
  if (window.layoutTestController) {
    window.layoutTestController.notifyDone();
  }
}

const RESULTS = {
  pass: 0,
  fail: 0,
};

// We cache these values since they will potentially be accessed many (100k+)
// times and accessing window can be significantly slower than a local variable.
const locationPathname = window.location.pathname;
const webglTestHarness = window.parent.webglTestHarness;

(function() {
  var WPT_TEST_ID = 0;

  // Store the current WPT test harness `test` function
  // if found, since it's overriden by some tests.
  var wpt_test = window.test;
  var wpt_assert_true = window.assert_true;
  var wt_async_test = window.async_test;

  window.reportTestResultsToHarness = function reportTestResultsToHarness(success, msg) {
    if (webglTestHarness) {
      webglTestHarness.reportResults(locationPathname, success, msg);
    } else if (wpt_test) { // WPT test harness
      wpt_test(function () {
        wpt_assert_true(success, msg);
      }, "WebGL test #" + (WPT_TEST_ID++));
    }
  }
}())

function reportSkippedTestResultsToHarness(success, msg) {
  if (webglTestHarness) {
    webglTestHarness.reportResults(locationPathname, success, msg, true);
  }
}

function notifyFinishedToHarness() {
  if (window._didNotifyFinishedToHarness) {
    testFailed("Duplicate notifyFinishedToHarness()");
  }
  window._didNotifyFinishedToHarness = true;

  if (webglTestHarness) {
    webglTestHarness.notifyFinished(locationPathname);
  }
  if (window.nonKhronosFrameworkNotifyDone) {
    window.nonKhronosFrameworkNotifyDone();
  }
}

(function () {
  var t = async_test("Overall test");
  var done = t.step_func_done(notifyFinishedToHarness);
  window.notifyFinishedToHarness = done;
  window.addEventListener("error", done);
}())

var _bufferedConsoleLogs = [];

function _bufferedLogToConsole(msg)
{
  if (_bufferedConsoleLogs) {
    _bufferedConsoleLogs.push(msg);
  } else if (window.console) {
    window.console.log(msg);
  }
}

// Public entry point exposed to many other files.
function bufferedLogToConsole(msg)
{
  _bufferedLogToConsole(msg);
}

// Called implicitly by testFailed().
function _flushBufferedLogsToConsole()
{
  if (_bufferedConsoleLogs) {
    if (window.console) {
      for (var ii = 0; ii < _bufferedConsoleLogs.length; ++ii) {
        window.console.log(_bufferedConsoleLogs[ii]);
      }
    }
    _bufferedConsoleLogs = null;
  }
}

var _jsTestPreVerboseLogging = true;

function enableJSTestPreVerboseLogging()
{
    _jsTestPreVerboseLogging = true;
}

function description(msg)
{
    initTestingHarness();
    if (msg === undefined) {
      msg = document.title;
    }
    _bufferedLogToConsole("DESCRIPTION: " + msg);
}

function _addSpan(contents)
{
}

function debug(msg)
{
    if (!quietMode())
      _addSpan(msg);
    if (_jsTestPreVerboseLogging) {
        _bufferedLogToConsole(msg);
    }
}

function escapeHTML(text)
{
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;");
}
/**
 * Defines the exception type for a test failure.
 * @constructor
 * @param {string} message The error message.
 */
var TestFailedException = function (message) {
   this.message = message;
   this.name = "TestFailedException";
};

/**
 * @param  {string=} msg
 */
function testPassed(msg) {
    msg = msg || 'Passed';
    if (_currentTestName)
      msg = _currentTestName + ': ' + msg;

    reportTestResultsToHarness(true, msg);

    if (!quietMode())
      _addSpan('<span><span class="pass">PASS</span> ' + escapeHTML(msg) + '</span>');
    if (_jsTestPreVerboseLogging) {
        _bufferedLogToConsole('PASS ' + msg);
    }
}

/**
 * @param  {string=} msg
 */
function testFailed(msg) {
    msg = msg || 'Failed';
    if (_currentTestName)
      msg = _currentTestName + ': ' + msg;

    reportTestResultsToHarness(false, msg);
    _addSpan('<span><span class="fail">FAIL</span> ' + escapeHTML(msg) + '</span>');
    _bufferedLogToConsole('FAIL ' + msg);
    _flushBufferedLogsToConsole();
}

var _currentTestName;

/**
 * Sets the current test name for usage within testPassedOptions/testFailedOptions.
 * @param {string=} name The name to set as the current test name.
 */
function setCurrentTestName(name)
{
    _currentTestName = name;
}

/**
 * Gets the current test name in use within testPassedOptions/testFailedOptions.
 * @return {string} The name of the current test.
 */
function getCurrentTestName()
{
    return _currentTestName;
}

/**
 * Variation of the testPassed function, with the option to not show (and thus not count) the test's pass result.
 * @param {string} msg The message to be shown in the pass result.
 * @param {boolean} addSpan Indicates whether the message will be visible (thus counted in the results) or not.
 */
function testPassedOptions(msg, addSpan)
{
    reportTestResultsToHarness(true, _currentTestName + ": " + msg);
    if (addSpan && !quietMode())
    {
        _addSpan('<span><span class="pass">PASS</span> ' + escapeHTML(_currentTestName) + ": " + escapeHTML(msg) + '</span>');
    }
    if (_jsTestPreVerboseLogging) {
        _bufferedLogToConsole('PASS ' + msg);
    }
}

/**
 * Report skipped tests.
 * @param {string} msg The message to be shown in the skip result.
 * @param {boolean} addSpan Indicates whether the message will be visible (thus counted in the results) or not.
 */
function testSkippedOptions(msg, addSpan)
{
    reportSkippedTestResultsToHarness(true, _currentTestName + ": " + msg);
    if (addSpan && !quietMode())
    {
        _addSpan('<span><span class="warn">SKIP</span> ' + escapeHTML(_currentTestName) + ": " + escapeHTML(msg) + '</span>');
    }
    if (_jsTestPreVerboseLogging) {
        _bufferedLogToConsole('SKIP' + msg);
    }
}

/**
 * Variation of the testFailed function, with the option to throw an exception or not.
 * @param {string} msg The message to be shown in the fail result.
 * @param {boolean} exthrow Indicates whether the function will throw a TestFailedException or not.
 */
function testFailedOptions(msg, exthrow)
{
    reportTestResultsToHarness(false, _currentTestName + ": " + msg);
    _addSpan('<span><span class="fail">FAIL</span> ' + escapeHTML(_currentTestName) + ": " + escapeHTML(msg) + '</span>');
    _bufferedLogToConsole('FAIL ' + msg);
    _flushBufferedLogsToConsole();
    if (exthrow) {
        _currentTestName = ""; //Remembering to set the name of current testcase to empty string.
        throw new TestFailedException(msg);
    }
}

function areArraysEqual(_a, _b)
{
    try {
        if (_a.length !== _b.length)
            return false;
        for (var i = 0; i < _a.length; i++)
            if (_a[i] !== _b[i])
                return false;
    } catch (ex) {
        return false;
    }
    return true;
}

function isMinusZero(n)
{
    // the only way to tell 0 from -0 in JS is the fact that 1/-0 is
    // -Infinity instead of Infinity
    return n === 0 && 1/n < 0;
}

function isResultCorrect(_actual, _expected)
{
    if (_expected === 0)
        return _actual === _expected && (1/_actual) === (1/_expected);
    if (_actual === _expected)
        return true;
    if (typeof(_expected) == "number" && isNaN(_expected))
        return typeof(_actual) == "number" && isNaN(_actual);
    if (Object.prototype.toString.call(_expected) == Object.prototype.toString.call([]))
        return areArraysEqual(_actual, _expected);
    return false;
}

function stringify(v)
{
    if (v === 0 && 1/v < 0)
        return "-0";
    else return "" + v;
}

function evalAndLog(_a)
{
  if (typeof _a != "string")
    debug("WARN: tryAndLog() expects a string argument");

  // Log first in case things go horribly wrong or this causes a sync event.
  debug(_a);

  var _av;
  try {
     _av = eval(_a);
  } catch (e) {
    testFailed(_a + " threw exception " + e);
  }
  return _av;
}

function shouldBeString(evalable, expected) {
    const val = eval(evalable);
    const text = evalable + " should be " + expected + ".";
    if (val == expected) {
        testPassed(text);
    } else {
        testFailed(text + " (was " + val + ")");
    }
}

function shouldBe(_a, _b, quiet)
{
    if (typeof _a != "string" || typeof _b != "string")
        debug("WARN: shouldBe() expects string arguments");
    var exception;
    var _av;
    try {
        _av = eval(_a);
    } catch (e) {
        exception = e;
    }
    var _bv = eval(_b);

    if (exception)
        testFailed(_a + " should be " + _bv + ". Threw exception " + exception);
    else if (isResultCorrect(_av, _bv)) {
        if (!quiet) {
            testPassed(_a + " is " + _b);
        }
    } else if (typeof(_av) == typeof(_bv))
        testFailed(_a + " should be " + _bv + ". Was " + stringify(_av) + ".");
    else
        testFailed(_a + " should be " + _bv + " (of type " + typeof _bv + "). Was " + _av + " (of type " + typeof _av + ").");
}

function shouldNotBe(_a, _b, quiet)
{
    if (typeof _a != "string" || typeof _b != "string")
        debug("WARN: shouldNotBe() expects string arguments");
    var exception;
    var _av;
    try {
        _av = eval(_a);
    } catch (e) {
        exception = e;
    }
    var _bv = eval(_b);

    if (exception)
        testFailed(_a + " should not be " + _bv + ". Threw exception " + exception);
    else if (!isResultCorrect(_av, _bv)) {
        if (!quiet) {
            testPassed(_a + " is not " + _b);
        }
    } else
        testFailed(_a + " should not be " + _bv + ".");
}

function shouldBeTrue(_a) { shouldBe(_a, "true"); }
function shouldBeFalse(_a) { shouldBe(_a, "false"); }
function shouldBeNaN(_a) { shouldBe(_a, "NaN"); }
function shouldBeNull(_a) { shouldBe(_a, "null"); }

function shouldBeEqualToString(a, b)
{
  var unevaledString = '"' + b.replace(/"/g, "\"") + '"';
  shouldBe(a, unevaledString);
}

function shouldEvaluateTo(actual, expected) {
  // A general-purpose comparator.  'actual' should be a string to be
  // evaluated, as for shouldBe(). 'expected' may be any type and will be
  // used without being eval'ed.
  if (expected == null) {
    // Do this before the object test, since null is of type 'object'.
    shouldBeNull(actual);
  } else if (typeof expected == "undefined") {
    shouldBeUndefined(actual);
  } else if (typeof expected == "function") {
    // All this fuss is to avoid the string-arg warning from shouldBe().
    try {
      var actualValue = eval(actual);
    } catch (e) {
      testFailed("Evaluating " + actual + ": Threw exception " + e);
      return;
    }
    shouldBe("'" + actualValue.toString().replace(/\n/g, "") + "'",
             "'" + expected.toString().replace(/\n/g, "") + "'");
  } else if (typeof expected == "object") {
    shouldBeTrue(actual + " == '" + expected + "'");
  } else if (typeof expected == "string") {
    shouldBe(actual, expected);
  } else if (typeof expected == "boolean") {
    shouldBe("typeof " + actual, "'boolean'");
    if (expected)
      shouldBeTrue(actual);
    else
      shouldBeFalse(actual);
  } else if (typeof expected == "number") {
    shouldBe(actual, stringify(expected));
  } else {
    debug(expected + " is unknown type " + typeof expected);
    shouldBeTrue(actual, "'"  +expected.toString() + "'");
  }
}

function shouldBeNonZero(_a)
{
  var exception;
  var _av;
  try {
     _av = eval(_a);
  } catch (e) {
     exception = e;
  }

  if (exception)
    testFailed(_a + " should be non-zero. Threw exception " + exception);
  else if (_av != 0)
    testPassed(_a + " is non-zero.");
  else
    testFailed(_a + " should be non-zero. Was " + _av);
}

function shouldBeNonNull(_a)
{
  var exception;
  var _av;
  try {
     _av = eval(_a);
  } catch (e) {
     exception = e;
  }

  if (exception)
    testFailed(_a + " should be non-null. Threw exception " + exception);
  else if (_av != null)
    testPassed(_a + " is non-null.");
  else
    testFailed(_a + " should be non-null. Was " + _av);
}

function shouldBeUndefined(_a)
{
  var exception;
  var _av;
  try {
     _av = eval(_a);
  } catch (e) {
     exception = e;
  }

  if (exception)
    testFailed(_a + " should be undefined. Threw exception " + exception);
  else if (typeof _av == "undefined")
    testPassed(_a + " is undefined.");
  else
    testFailed(_a + " should be undefined. Was " + _av);
}

function shouldBeDefined(_a)
{
  var exception;
  var _av;
  try {
     _av = eval(_a);
  } catch (e) {
     exception = e;
  }

  if (exception)
    testFailed(_a + " should be defined. Threw exception " + exception);
  else if (_av !== undefined)
    testPassed(_a + " is defined.");
  else
    testFailed(_a + " should be defined. Was " + _av);
}

function shouldBeLessThanOrEqual(_a, _b) {
    if (typeof _a != "string" || typeof _b != "string")
        debug("WARN: shouldBeLessThanOrEqual expects string arguments");

    var exception;
    var _av;
    try {
        _av = eval(_a);
    } catch (e) {
        exception = e;
    }
    var _bv = eval(_b);

    if (exception)
        testFailed(_a + " should be <= " + _b + ". Threw exception " + exception);
    else if (typeof _av == "undefined" || _av > _bv)
        testFailed(_a + " should be >= " + _b + ". Was " + _av + " (of type " + typeof _av + ").");
    else
        testPassed(_a + " is <= " + _b);
}

function shouldBeGreaterThanOrEqual(_a, _b) {
    if (typeof _a != "string" || typeof _b != "string")
        debug("WARN: shouldBeGreaterThanOrEqual expects string arguments");

    var exception;
    var _av;
    try {
        _av = eval(_a);
    } catch (e) {
        exception = e;
    }
    var _bv = eval(_b);

    if (exception)
        testFailed(_a + " should be >= " + _b + ". Threw exception " + exception);
    else if (typeof _av == "undefined" || _av < _bv)
        testFailed(_a + " should be >= " + _b + ". Was " + _av + " (of type " + typeof _av + ").");
    else
        testPassed(_a + " is >= " + _b);
}

function expectTrue(v, msg) {
  if (v) {
    testPassed(msg);
  } else {
    testFailed(msg);
  }
}

function maxArrayDiff(a, b) {
    if (a.length != b.length)
        throw new Error(`a and b have different lengths: ${a.length} vs ${b.length}`);

    let diff = 0;
    for (const i in a) {
        diff = Math.max(diff, Math.abs(a[i] - b[i]));
    }
    return diff;
}

function expectArray(was, expected, maxDiff=0) {
    const diff = maxArrayDiff(expected, was);
    let str = `Expected [${expected.toString()}]`;
    let fn = testPassed;
    if (maxDiff) {
        str += ' +/- ' + maxDiff;
    }
    if (diff > maxDiff) {
        fn = testFailed;
        str += `, was [${was.toString()}]`;
    }
    fn(str);
}

function shouldThrow(_a, _e)
{
  var exception;
  var _av;
  try {
     _av = eval(_a);
  } catch (e) {
     exception = e;
  }

  var _ev;
  if (_e)
      _ev =  eval(_e);

  if (exception) {
    if (typeof _e == "undefined" || exception == _ev)
      testPassed(_a + " threw exception " + exception + ".");
    else
      testFailed(_a + " should throw " + (typeof _e == "undefined" ? "an exception" : _ev) + ". Threw exception " + exception + ".");
  } else if (typeof _av == "undefined")
    testFailed(_a + " should throw " + (typeof _e == "undefined" ? "an exception" : _ev) + ". Was undefined.");
  else
    testFailed(_a + " should throw " + (typeof _e == "undefined" ? "an exception" : _ev) + ". Was " + _av + ".");
}

function shouldNotThrow(evalStr, desc) {
  desc = desc || `\`${evalStr}\``;
  try {
    eval(evalStr);
    testPassed(`${desc} should not throw.`);
  } catch (e) {
    testFailed(`${desc} should not throw, but threw exception ${e}.`);
  }
}


function shouldBeType(_a, _type) {
    var exception;
    var _av;
    try {
        _av = eval(_a);
    } catch (e) {
        exception = e;
    }

    var _typev = eval(_type);

    if(_typev === Number){
        if(_av instanceof Number){
            testPassed(_a + " is an instance of Number");
        }
        else if(typeof(_av) === 'number'){
            testPassed(_a + " is an instance of Number");
        }
        else{
            testFailed(_a + " is not an instance of Number");
        }
    }
    else if (_av instanceof _typev) {
        testPassed(_a + " is an instance of " + _type);
    } else {
        testFailed(_a + " is not an instance of " + _type);
    }
}

/**
 * Shows a message in case expression test fails.
 * @param {boolean} exp
 * @param {straing} message
 */
function checkMessage(exp, message) {
    if ( !exp )
        _addSpan('<span><span class="warn">WARNING</span> ' + escapeHTML(_currentTestName) + ": " + escapeHTML(message) + '</span>');
}

function assertMsg(assertion, msg) {
    if (assertion) {
        testPassed(msg);
    } else {
        testFailed(msg);
    }
}

/**
 * Variation of the assertMsg function, with the option to not show (and thus not count) the test's pass result,
 * and throw or not a TestFailedException in case of failure.
 * @param {boolean} assertion If this is true, means success, else failure.
 * @param {?string} msg The message to be shown in the result.
 * @param {boolean} verbose In case of success, determines if the test will show it's result and count in the results.
 * @param {boolean} exthrow In case of failure, determines if the function will throw a TestFailedException.
 */
function assertMsgOptions(assertion, msg, verbose, exthrow) {
    if (assertion) {
        testPassedOptions(msg, verbose);
    } else {
        testFailedOptions(msg, exthrow);
    }
}


function webglHarnessCollectGarbage() {
    if (window.GCController) {
        window.GCController.collect();
        return;
    }

    if (window.opera && window.opera.collect) {
        window.opera.collect();
        return;
    }

    try {
        window.QueryInterface(Components.interfaces.nsIInterfaceRequestor)
              .getInterface(Components.interfaces.nsIDOMWindowUtils)
              .garbageCollect();
        return;
    } catch(e) {}

    if (window.gc) {
        window.gc();
        return;
    }

    if (window.CollectGarbage) {
        CollectGarbage();
        return;
    }

    // WebKit's MiniBrowser.
    if (window.$vm) {
        window.$vm.gc();
        return;
    }

    function gcRec(n) {
        if (n < 1)
            return {};
        var temp = {i: "ab" + i + (i / 100000)};
        temp += "foo";
        gcRec(n-1);
    }
    for (var i = 0; i < 1000; i++)
        gcRec(10);
}

function finishTest() {
  successfullyParsed = true;
  var epilogue = document.createElement("script");
  var basePath = "";
  var expectedBase = "js-test-pre.js";
  var scripts = document.getElementsByTagName('script');
  for (var script, i = 0; script = scripts[i]; i++) {
    var src = script.src;
    var l = src.length;
    if (src.substr(l - expectedBase.length) == expectedBase) {
      basePath = src.substr(0, l - expectedBase.length);
      break;
    }
  }
  epilogue.src = basePath + "js-test-post.js";
  document.body.appendChild(epilogue);
}

/// Prefer `call(() => { ... })` to `(() => { ... })()`\
/// This way, it's clear up-front that we're calling not just defining.
function call(fn) {
    return fn();
}

/// `for (const i of range(3))` => 0, 1, 2
/// Don't use `for...in range(n)`, it will not work.
function* range(n) {
  for (let i = 0; i < n; i++) {
    yield i;
  }
}

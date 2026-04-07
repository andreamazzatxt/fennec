import ApplicationServices
import AppKit
import Foundation

print("=== Fennec AX C-API Diagnostic ===")
print("You have 4 seconds — focus a text field and SELECT some text...\n")

for i in stride(from: 4, through: 1, by: -1) {
    print("  \(i)...")
    Thread.sleep(forTimeInterval: 1)
}
print()

let system = AXUIElementCreateSystemWide()

// Test 1: AXFocusedApplication from system-wide
print("--- Test 1: AXFocusedApplication (C API) ---")
var focusedApp: CFTypeRef?
let appErr = AXUIElementCopyAttributeValue(system, "AXFocusedApplication" as CFString, &focusedApp)
if appErr == .success, focusedApp != nil {
    print("  OK: got focused app element")
} else {
    print("  FAILED: AXError = \(appErr.rawValue)")
}

// Test 2: NSWorkspace frontmostApplication PID
print("\n--- Test 2: NSWorkspace frontmostApplication ---")
let frontApp = NSWorkspace.shared.frontmostApplication
let pid = frontApp?.processIdentifier ?? -1
let appName = frontApp?.localizedName ?? "unknown"
print("  App: \(appName) (PID: \(pid))")

// Test 3: AXUIElementCreateApplication(pid) + AXFocusedUIElement
print("\n--- Test 3: AXFocusedUIElement from CreateApplication(pid) ---")
let appFromPid = AXUIElementCreateApplication(pid)
var focusedElem: CFTypeRef?
let elemErr = AXUIElementCopyAttributeValue(appFromPid, "AXFocusedUIElement" as CFString, &focusedElem)
if elemErr == .success, focusedElem != nil {
    let elem = focusedElem as! AXUIElement
    var role: CFTypeRef?
    AXUIElementCopyAttributeValue(elem, "AXRole" as CFString, &role)
    print("  OK: role = \(role ?? "unknown" as CFTypeRef)")
} else {
    print("  FAILED: AXError = \(elemErr.rawValue)")
}

// Test 4: AXFocusedUIElement from system-wide
print("\n--- Test 4: AXFocusedUIElement from system-wide ---")
var focusedElem2: CFTypeRef?
let elemErr2 = AXUIElementCopyAttributeValue(system, "AXFocusedUIElement" as CFString, &focusedElem2)
if elemErr2 == .success, focusedElem2 != nil {
    let elem = focusedElem2 as! AXUIElement
    var role: CFTypeRef?
    AXUIElementCopyAttributeValue(elem, "AXRole" as CFString, &role)
    print("  OK: role = \(role ?? "unknown" as CFTypeRef)")
} else {
    print("  FAILED: AXError = \(elemErr2.rawValue)")
}

// Test 5: If test 3 worked, try reading AXValue
if elemErr == .success, let elem = focusedElem {
    print("\n--- Test 5: AXValue from focused element ---")
    let axElem = elem as! AXUIElement
    var value: CFTypeRef?
    let valErr = AXUIElementCopyAttributeValue(axElem, "AXValue" as CFString, &value)
    if valErr == .success, let v = value as? String {
        print("  OK: \(v.count) chars")
    } else {
        print("  FAILED: AXError = \(valErr.rawValue)")
    }

    print("\n--- Test 6: AXSelectedText from focused element ---")
    var selText: CFTypeRef?
    let selErr = AXUIElementCopyAttributeValue(axElem, "AXSelectedText" as CFString, &selText)
    if selErr == .success, let s = selText as? String {
        print("  OK: '\(s)'")
    } else {
        print("  FAILED: AXError = \(selErr.rawValue)")
    }
}

print("\n=== Done ===")

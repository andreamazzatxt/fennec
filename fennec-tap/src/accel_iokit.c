#include <IOKit/IOKitLib.h>
#include <IOKit/hid/IOHIDDevice.h>
#include <CoreFoundation/CoreFoundation.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>

#define PAGE_VENDOR       0xFF00
#define USAGE_ACCEL       3
#define USAGE_FALLBACK    255
#define MAX_DEVICES       8
#define REPORT_BUF_SIZE   4096
#define REPORT_INTERVAL_US 1000
#define IMU_REPORT_LEN    22
#define IMU_DATA_OFFSET   6

/* ── state ── */
static IOHIDDeviceRef g_devices[MAX_DEVICES];
static uint8_t        g_report_bufs[MAX_DEVICES][REPORT_BUF_SIZE];
static int64_t        g_device_usage[MAX_DEVICES];
static int            g_device_count = 0;
static int            g_callback_count = 0;

/* Rust callback */
typedef void (*rust_cb_t)(void *ctx, double x, double y, double z);
static rust_cb_t g_rust_cb = NULL;
static void     *g_rust_ctx = NULL;

/* ── helpers ── */

static int64_t get_int_property(io_service_t svc, CFStringRef key) {
    int64_t val = 0;
    CFTypeRef prop = IORegistryEntryCreateCFProperty(svc, key, kCFAllocatorDefault, 0);
    if (prop) {
        CFNumberGetValue(prop, kCFNumberSInt64Type, &val);
        CFRelease(prop);
    }
    return val;
}

static void set_int_property(io_service_t svc, CFStringRef key, int32_t value) {
    CFNumberRef num = CFNumberCreate(NULL, kCFNumberSInt32Type, &value);
    if (num) {
        IORegistryEntrySetCFProperty(svc, key, num);
        CFRelease(num);
    }
}

static int is_accel_usage(int64_t usage) {
    return usage == USAGE_ACCEL || usage == USAGE_FALLBACK;
}

/* ── HID callback ── */

static void accel_callback(void *context, IOReturn result, void *sender,
                           IOHIDReportType type, uint32_t reportID,
                           uint8_t *report, CFIndex reportLength) {
    (void)result; (void)sender; (void)type; (void)context;

    g_callback_count++;

    if (reportLength < IMU_REPORT_LEN) return;

    int32_t x_raw, y_raw, z_raw;
    memcpy(&x_raw, report + IMU_DATA_OFFSET,     4);
    memcpy(&y_raw, report + IMU_DATA_OFFSET + 4,  4);
    memcpy(&z_raw, report + IMU_DATA_OFFSET + 8,  4);

    double x = (double)x_raw / 65536.0;
    double y = (double)y_raw / 65536.0;
    double z = (double)z_raw / 65536.0;

    /* Log first few samples */
    if (g_callback_count <= 10) {
        fprintf(stderr, "accel: #%d len=%ld x=%.4f y=%.4f z=%.4f\n",
                g_callback_count, (long)reportLength, x, y, z);
    }

    if (g_rust_cb) {
        g_rust_cb(g_rust_ctx, x, y, z);
    }
}

/* ── wake SPU drivers ── */

static int wake_spu_drivers(void) {
    CFMutableDictionaryRef matching = IOServiceMatching("AppleSPUHIDDriver");
    io_iterator_t iter;
    kern_return_t kr = IOServiceGetMatchingServices(0, matching, &iter);
    if (kr != KERN_SUCCESS) {
        fprintf(stderr, "accel: AppleSPUHIDDriver matching failed (%d)\n", kr);
        return -1;
    }
    int count = 0;
    io_service_t svc;
    while ((svc = IOIteratorNext(iter)) != 0) {
        set_int_property(svc, CFSTR("SensorPropertyReportingState"), 1);
        set_int_property(svc, CFSTR("SensorPropertyPowerState"), 1);
        set_int_property(svc, CFSTR("ReportInterval"), REPORT_INTERVAL_US);
        IOObjectRelease(svc);
        count++;
    }
    IOObjectRelease(iter);
    fprintf(stderr, "accel: woke %d SPU drivers\n", count);
    return 0;
}

/* ── register HID devices ── */

static int register_hid_devices(void) {
    CFMutableDictionaryRef matching = IOServiceMatching("AppleSPUHIDDevice");
    io_iterator_t iter;
    kern_return_t kr = IOServiceGetMatchingServices(0, matching, &iter);
    if (kr != KERN_SUCCESS) {
        fprintf(stderr, "accel: AppleSPUHIDDevice matching failed (%d)\n", kr);
        return -1;
    }

    int callbacks = 0;
    int total = 0;
    io_service_t svc;
    while ((svc = IOIteratorNext(iter)) != 0) {
        total++;
        int64_t up = get_int_property(svc, CFSTR("PrimaryUsagePage"));
        int64_t u  = get_int_property(svc, CFSTR("PrimaryUsage"));
        fprintf(stderr, "accel: device %d: page=0x%llx usage=%lld\n", total, up, u);

        if (up != PAGE_VENDOR) {
            IOObjectRelease(svc);
            continue;
        }

        IOHIDDeviceRef hid = IOHIDDeviceCreate(kCFAllocatorDefault, svc);
        if (!hid) {
            IOObjectRelease(svc);
            continue;
        }

        kr = IOHIDDeviceOpen(hid, kIOHIDOptionsTypeNone);
        if (kr != kIOReturnSuccess) {
            fprintf(stderr, "accel: open failed for device %d (0x%x)\n", total, kr);
            CFRelease(hid);
            IOObjectRelease(svc);
            continue;
        }

        if (g_device_count >= MAX_DEVICES) {
            CFRelease(hid);
            IOObjectRelease(svc);
            continue;
        }

        int idx = g_device_count++;
        g_devices[idx] = hid;
        g_device_usage[idx] = u;

        IOHIDDeviceScheduleWithRunLoop(hid, CFRunLoopGetCurrent(), kCFRunLoopDefaultMode);

        if (is_accel_usage(u)) {
            IOHIDDeviceRegisterInputReportCallback(
                hid, g_report_bufs[idx], REPORT_BUF_SIZE,
                accel_callback, (void *)(intptr_t)idx);
            callbacks++;
            fprintf(stderr, "accel: registered callback on device %d (usage=%lld)\n", total, u);
        }

        IOObjectRelease(svc);
    }
    IOObjectRelease(iter);
    fprintf(stderr, "accel: %d devices, %d callbacks\n", g_device_count, callbacks);
    return callbacks > 0 ? 0 : -1;
}

/* ── public API ── */

void accel_init(void) {
    memset(g_devices, 0, sizeof(g_devices));
    memset(g_report_bufs, 0, sizeof(g_report_bufs));
    g_device_count = 0;
    g_callback_count = 0;
}

void accel_set_callback(rust_cb_t cb, void *ctx) {
    g_rust_cb = cb;
    g_rust_ctx = ctx;
}

void accel_run(void) {
    if (wake_spu_drivers() != 0) {
        fprintf(stderr, "accel: failed to wake SPU drivers\n");
        return;
    }
    if (register_hid_devices() != 0) {
        fprintf(stderr, "accel: failed to register HID devices\n");
        return;
    }
    fprintf(stderr, "accel: entering run loop (%d devices)\n", g_device_count);
    while (1) {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.0, false);
    }
}

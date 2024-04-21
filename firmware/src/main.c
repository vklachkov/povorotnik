#include "pico/stdlib.h"
#include "pico/cyw43_arch.h"

#include "hardware/i2c.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "profile.h"
#include "btstack.h"

#define TRY(x) { int status = (x); if (status < 0) return status; }

#define ADXL345_ADDRESS (0xA6 >> 1)
#define ADXL345_TIMEOUT make_timeout_time_ms(100)

#define HEARTBEAT_PERIOD_MS 1000

static int  le_notification_enabled;
static btstack_timer_source_t heartbeat;
static btstack_packet_callback_registration_t hci_event_callback_registration;
static hci_con_handle_t con_handle;

static void packet_handler (uint8_t packet_type, uint16_t channel, uint8_t *packet, uint16_t size);
static uint16_t att_read_callback(hci_con_handle_t con_handle, uint16_t att_handle, uint16_t offset, uint8_t * buffer, uint16_t buffer_size);
static int att_write_callback(hci_con_handle_t con_handle, uint16_t att_handle, uint16_t transaction_mode, uint16_t offset, uint8_t *buffer, uint16_t buffer_size);
static void  heartbeat_handler(struct btstack_timer_source *ts);
static void beat(void);

// Flags general discoverable, BR/EDR supported (== not supported flag not set) when ENABLE_GATT_OVER_CLASSIC is enabled
#define APP_AD_FLAGS 0x06

const uint8_t adv_data[] = {
    // Flags general discoverable
    0x02, BLUETOOTH_DATA_TYPE_FLAGS, APP_AD_FLAGS,
    // Name
    0x0b, BLUETOOTH_DATA_TYPE_COMPLETE_LOCAL_NAME, 'L', 'E', ' ', 'C', 'o', 'u', 'n', 't', 'e', 'r', 
    // Incomplete List of 16-bit Service Class UUIDs -- FF10 - only valid for testing!
    0x03, BLUETOOTH_DATA_TYPE_INCOMPLETE_LIST_OF_16_BIT_SERVICE_CLASS_UUIDS, 0x10, 0xff,
};
const uint8_t adv_data_len = sizeof(adv_data);

/* LISTING_END */

/*
 * @section Heartbeat Handler
 *
 * @text The heartbeat handler updates the value of the single Characteristic provided in this example,
 * and request a ATT_EVENT_CAN_SEND_NOW to send a notification if enabled see Listing heartbeat.
 */

 /* LISTING_START(heartbeat): Hearbeat Handler */
static char accelerometer_value_json[256];
static int  accelerometer_value_json_len;

static void beat(void){
    int err = 0;
    
    const uint8_t read_values[] = { 0x32 };
    err = i2c_write_blocking_until(i2c_default, ADXL345_ADDRESS, read_values, sizeof(read_values), false, ADXL345_TIMEOUT);
    if (err < 0) {
        printf("Failed to request values from ADXL345. Status code: %d", err);
        return;
    }

    uint8_t values[6] = { 255, 255, 255, 255, 255, 255 };
    err = i2c_read_blocking_until(i2c_default, ADXL345_ADDRESS, values, sizeof(values), false, ADXL345_TIMEOUT);
    if (err < 0) {
        printf("Failed to read values from ADXL345. Status code: %d", err);
        return;
    }

    int16_t raw_x = ((int16_t)values[1] << 8) | (int16_t)values[0];
    int16_t raw_y = ((int16_t)values[3] << 8) | (int16_t)values[2];
    int16_t raw_z = ((int16_t)values[5] << 8) | (int16_t)values[4];

    float x = (float)raw_x / 32.0;
    float y = (float)raw_y / 32.0;
    float z = (float)raw_z / 32.0;

    accelerometer_value_json_len = snprintf(
        accelerometer_value_json,
        sizeof(accelerometer_value_json),
        "{\"x\":%f,\"y\":%f,\"z\":%f}",
        x, y, z
    );
}

static void heartbeat_handler(struct btstack_timer_source *ts){
    if (le_notification_enabled) {
        beat();
        att_server_request_can_send_now_event(con_handle);
    }

    btstack_run_loop_set_timer(ts, HEARTBEAT_PERIOD_MS);
    btstack_run_loop_add_timer(ts);
} 
/* LISTING_END */

/* 
 * @section Packet Handler
 *
 * @text The packet handler is used to:
 *        - stop the counter after a disconnect
 *        - send a notification when the requested ATT_EVENT_CAN_SEND_NOW is received
 */

/* LISTING_START(packetHandler): Packet Handler */
static void packet_handler (uint8_t packet_type, uint16_t channel, uint8_t *packet, uint16_t size){
    UNUSED(channel);
    UNUSED(size);

    if (packet_type != HCI_EVENT_PACKET) return;
    
    switch (hci_event_packet_get_type(packet)) {
        case HCI_EVENT_DISCONNECTION_COMPLETE:
            le_notification_enabled = 0;
            break;
        case ATT_EVENT_CAN_SEND_NOW:
            att_server_notify(con_handle, ATT_CHARACTERISTIC_0000FF11_0000_1000_8000_00805F9B34FB_01_VALUE_HANDLE, (uint8_t*) accelerometer_value_json, accelerometer_value_json_len);
            break;
        default:
            break;
    }
}

/* LISTING_END */

/*
 * @section ATT Read
 *
 * @text The ATT Server handles all reads to constant data. For dynamic data like the custom characteristic, the registered
 * att_read_callback is called. To handle long characteristics and long reads, the att_read_callback is first called
 * with buffer == NULL, to request the total value length. Then it will be called again requesting a chunk of the value.
 * See Listing attRead.
 */

/* LISTING_START(attRead): ATT Read */

// ATT Client Read Callback for Dynamic Data
// - if buffer == NULL, don't copy data, just return size of value
// - if buffer != NULL, copy data and return number bytes copied
// @param offset defines start of attribute value
static uint16_t att_read_callback(hci_con_handle_t connection_handle, uint16_t att_handle, uint16_t offset, uint8_t * buffer, uint16_t buffer_size){
    UNUSED(connection_handle);

    if (att_handle == ATT_CHARACTERISTIC_0000FF11_0000_1000_8000_00805F9B34FB_01_VALUE_HANDLE){
        return att_read_callback_handle_blob((const uint8_t *) accelerometer_value_json, accelerometer_value_json_len, offset, buffer, buffer_size);
    }
    return 0;
}
/* LISTING_END */


/*
 * @section ATT Write
 *
 * @text The only valid ATT writes in this example are to the Client Characteristic Configuration, which configures notification
 * and indication and to the the Characteristic Value.
 * If the ATT handle matches the client configuration handle, the new configuration value is stored and used
 * in the heartbeat handler to decide if a new value should be sent.
 * If the ATT handle matches the characteristic value handle, we print the write as hexdump
 * See Listing attWrite.
 */

/* LISTING_START(attWrite): ATT Write */
static int att_write_callback(hci_con_handle_t connection_handle, uint16_t att_handle, uint16_t transaction_mode, uint16_t offset, uint8_t *buffer, uint16_t buffer_size){
    switch (att_handle){
        case ATT_CHARACTERISTIC_0000FF11_0000_1000_8000_00805F9B34FB_01_CLIENT_CONFIGURATION_HANDLE:
            le_notification_enabled = little_endian_read_16(buffer, 0) == GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_NOTIFICATION;
            con_handle = connection_handle;
            break;
        case ATT_CHARACTERISTIC_0000FF11_0000_1000_8000_00805F9B34FB_01_VALUE_HANDLE:
            printf("Write: transaction mode %u, offset %u, data (%u bytes): ", transaction_mode, offset, buffer_size);
            printf_hexdump(buffer, buffer_size);
            break;
        default:
            break;
    }
    return 0;
}

void setup_i2c() {
    i2c_init(i2c_default, 100000);
    gpio_set_function(PICO_DEFAULT_I2C_SDA_PIN, GPIO_FUNC_I2C);
    gpio_pull_up(PICO_DEFAULT_I2C_SDA_PIN);
    gpio_set_function(PICO_DEFAULT_I2C_SCL_PIN, GPIO_FUNC_I2C);
    gpio_pull_up(PICO_DEFAULT_I2C_SCL_PIN);
}

int adxl345_init() {
    // Data format control.
    // Set g range to ±16g.
    const uint8_t setup_acceleration[] = { 0x31, 0x03 };
    TRY(i2c_write_blocking_until(i2c_default, ADXL345_ADDRESS, setup_acceleration, sizeof(setup_acceleration), false, ADXL345_TIMEOUT));

    // Enable power.
    // Set measure bit.
    const uint8_t enable_power[] = { 0x2D, 0x08 };
    TRY(i2c_write_blocking_until(i2c_default, ADXL345_ADDRESS, enable_power, sizeof(enable_power), false, ADXL345_TIMEOUT));

    // Set X offset.
    const uint8_t x_offset = 0;
    const uint8_t set_x_offset[] = { 0x1E, x_offset };
    TRY(i2c_write_blocking_until(i2c_default, ADXL345_ADDRESS, set_x_offset, sizeof(set_x_offset), false, ADXL345_TIMEOUT));

    // Set Y offset.
    const uint8_t y_offset = 0;
    const uint8_t set_y_offset[] = { 0x1F, y_offset };
    TRY(i2c_write_blocking_until(i2c_default, ADXL345_ADDRESS, set_y_offset, sizeof(set_y_offset), false, ADXL345_TIMEOUT));

    // Set Z offset.
    const uint8_t z_offset = 5;
    const uint8_t set_z_offset[] = { 0x20, z_offset };
    TRY(i2c_write_blocking_until(i2c_default, ADXL345_ADDRESS, set_z_offset, sizeof(set_z_offset), false, ADXL345_TIMEOUT));

    // Read device id.
    const uint8_t read_device_id[] = { 0x00 };
    TRY(i2c_write_blocking_until(i2c_default, ADXL345_ADDRESS, read_device_id, sizeof(read_device_id), false, ADXL345_TIMEOUT));

    uint8_t device_id = 0x00;
    TRY(i2c_read_blocking_until(i2c_default, ADXL345_ADDRESS, &device_id, 1, false, ADXL345_TIMEOUT));
    
    if (device_id == 0xE5) {
        return PICO_ERROR_NONE;
    } else {
        return PICO_ERROR_CONNECT_FAILED;
    }
}

int main() {
    int err = 0;

    // Initialize SDK.
    stdio_init_all();

    // Setup pins.
    setup_i2c();

    // Setup peripherals.
    err = adxl345_init();
    if (err < 0) {
        printf("Failed to setup ADXL435. Status code: %d", err);
        return -1;
    } 

    err = cyw43_arch_init();
    if (err > 0) {
        printf("Failed to initialize CYW43. Status code: %d", err);
        return -1;
    }

    // Initialize BLE.
    l2cap_init();
    sm_init();

    // Setup ATT server.
    att_server_init(profile_data, att_read_callback, att_write_callback);    

    // Setup advertisement.
    uint16_t adv_int_min = 0x0030;
    uint16_t adv_int_max = 0x0030;
    uint8_t adv_type = 0;
    bd_addr_t null_addr;
    memset(null_addr, 0, 6);
    gap_advertisements_set_params(adv_int_min, adv_int_max, adv_type, 0, null_addr, 0x07, 0x00);
    gap_advertisements_set_data(adv_data_len, (uint8_t*) adv_data);
    gap_advertisements_enable(1);

    // Register for HCI events.
    hci_event_callback_registration.callback = &packet_handler;
    hci_add_event_handler(&hci_event_callback_registration);

    // Register for ATT events.
    att_server_register_packet_handler(packet_handler);

    // set one-shot timer
    heartbeat.process = &heartbeat_handler;
    btstack_run_loop_set_timer(&heartbeat, HEARTBEAT_PERIOD_MS);
    btstack_run_loop_add_timer(&heartbeat);

    // beat once
    beat();

    // Let's go.
    hci_power_control(HCI_POWER_ON);
    while (true) { sleep_ms(1000); }
}
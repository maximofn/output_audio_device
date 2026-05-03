#!/usr/bin/env python3
import signal
import gi
gi.require_version('AppIndicator3', '0.1')
from gi.repository import AppIndicator3, GLib
from gi.repository import Gtk as gtk
import os
import subprocess
import webbrowser
import time
import argparse

APPINDICATOR_ID = 'Audio_output_devices'

PATH = os.path.dirname(os.path.realpath(__file__))
ICON_PATH = os.path.abspath(f"{PATH}/speaker.png")

actual_time_menu_item = None
number_of_devices_in_menu = 0
output_audio_devices_items = []

def main(debug=False):
    audio_output_devices_indicator = AppIndicator3.Indicator.new(APPINDICATOR_ID, ICON_PATH, AppIndicator3.IndicatorCategory.SYSTEM_SERVICES)
    audio_output_devices_indicator.set_status(AppIndicator3.IndicatorStatus.ACTIVE)
    if debug: print("\n Output audio devices:")
    audio_output_devices_indicator.set_menu(build_menu())

    # Get CPU info
    GLib.timeout_add_seconds(1, update_output_audio_devices, audio_output_devices_indicator, debug)

    GLib.MainLoop().run()

def open_repo_link(_):
    webbrowser.open('https://github.com/maximofn/output_audio_device')

def buy_me_a_coffe(_):
    webbrowser.open('https://www.buymeacoffee.com/maximofn')

def change_output_device(item, output_device_id):
    subprocess.run(["pactl", "set-default-sink", output_device_id])

def build_menu(debug=False):
    global actual_time_menu_item
    global number_of_devices_in_menu
    global output_audio_devices_items

    menu = gtk.Menu()

    item_output_devices_title = gtk.MenuItem(label='Output devices')
    menu.append(item_output_devices_title)

    output_devices = get_output_audio_devices(debug)
    output_audio_devices_items = []
    active_output_audio_device = get_active_output_audio_device(debug)
    for output_device in output_devices:
        if 'Name' in output_device.keys():
            key_name = 'Name'
        elif 'Nombre' in output_device.keys():
            key_name = 'Nombre'
        if 'Description' in output_device.keys():
            key_description = 'Description'
        elif 'Descripción' in output_device.keys():
            key_description = 'Descripción'
        elif 'Descripcion' in output_device.keys():
            key_description = 'Descripcion'
        if not active_output_audio_device:
            output_device_item = gtk.MenuItem(label=f"\t{output_device[key_description]}")
        else:
            if output_device[key_name] in active_output_audio_device:
                output_device_item = gtk.MenuItem(label=f"\t(active) {output_device[key_description]}")
            else:
                output_device_item = gtk.MenuItem(label=f"\t{output_device[key_description]}")
        output_device_item.connect('activate', change_output_device, output_device['id'])
        menu.append(output_device_item)
        output_audio_devices_items.append(output_device_item)
    number_of_devices_in_menu = len(output_devices)

    horizontal_separator1 = gtk.SeparatorMenuItem()
    menu.append(horizontal_separator1)

    actual_time_menu_item = gtk.MenuItem(label=time.strftime("%H:%M:%S"))
    menu.append(actual_time_menu_item)

    horizontal_separator2 = gtk.SeparatorMenuItem()
    menu.append(horizontal_separator2)

    item_repo = gtk.MenuItem(label='Repository')
    item_repo.connect('activate', open_repo_link)
    menu.append(item_repo)

    item_buy_me_a_coffe = gtk.MenuItem(label='Buy me a coffe')
    item_buy_me_a_coffe.connect('activate', buy_me_a_coffe)
    menu.append(item_buy_me_a_coffe)

    horizontal_separator3 = gtk.SeparatorMenuItem()
    menu.append(horizontal_separator3)

    item_quit = gtk.MenuItem(label='Quit')
    item_quit.connect('activate', quit)
    menu.append(item_quit)

    menu.show_all()
    return menu

def update_menu(indicator, output_devices, debug=False):
    actual_time_menu_item.set_label(time.strftime("%H:%M:%S"))

    # If the number of devices has changed, update the menu
    if len (output_devices) != number_of_devices_in_menu:
        indicator.set_menu(build_menu(debug))

    # If the number of devices has not changed, update the devices
    else:
        active_output_audio_device = get_active_output_audio_device(debug)
        for number_output_device, output_device in enumerate(output_devices):
            if 'Name' in output_device.keys():
                key_name = 'Name'
            elif 'Nombre' in output_device.keys():
                key_name = 'Nombre'
            if 'Description' in output_device.keys():
                key_description = 'Description'
            elif 'Descripción' in output_device.keys():
                key_description = 'Descripción'
            elif 'Descripcion' in output_device.keys():
                key_description = 'Descripcion'
            if not active_output_audio_device:
                output_audio_devices_items[number_output_device].set_label(f"\t{output_device[key_description]}")
            else:
                if output_device[key_name] in active_output_audio_device:
                    output_audio_devices_items[number_output_device].set_label(f"\t(active) {output_device[key_description]}")
                else:
                    output_audio_devices_items[number_output_device].set_label(f"\t{output_device[key_description]}")

def update_output_audio_devices(indicator, debug=False):
    if debug: print("\n Output audio devices:")
    output_devices = get_output_audio_devices(debug)
    update_menu(indicator, output_devices, debug)

    return True

def get_active_output_audio_device(debug=False):
    # Get output audio devices
    result = subprocess.run(["pactl", "get-default-sink"], capture_output=True, text=True)
    if result:
        active_output_audio_device = result.stdout.strip()
        if len(active_output_audio_device) > 0:
            return active_output_audio_device
    
    result = subprocess.run(["pactl", "list", "sinks"], capture_output=True, text=True)
    if result:
        output = result.stdout
        active_output_audio_device = None
        for i, line in enumerate(output.split("\n")):
            if "State" in line or "Estado" in line:
                state = line.split(":")[1].strip()
                if state == "RUNNING":
                    active_output_audio_device = output.split("\n")[i + 1].split(":")[1].strip()
                    return active_output_audio_device
    return None

def get_output_audio_devices(debug=False):
    # Get output audio devices
    result = subprocess.run(["pactl", "list", "sinks"], capture_output=True, text=True)
    if result:
        output = result.stdout
        output_devices = []
        output_device = None
        for number_line, line in enumerate(output.split("\n")):
            if "Sink #" in line or "Destino #" in line:
                if output_device:
                    output_devices.append(output_device)
                output_device = {"id": line.split("#")[1]}
                if debug: print(f"\tOutput device: {output_device['id']}")
                properties = 0
                ports = 0
                formats = 0
            elif "Properties:" in line:
                properties = 1
                ports = 0
                formats = 0
                output_device["properties"] = {}
            elif "Ports:" in line:
                properties = 0
                ports = 1
                formats = 0
                output_device["ports"] = {}
            elif "Formats:" in line:
                properties = 0
                ports = 0
                formats = 1
                output_device["formats"] = {}
            else:
                key = line.split(":")[0].strip()
                if "balance" in key:
                    key = "balance"
                    value = line.split("balance")[1]
                else:
                    value = line.split(":")[1:]

                # if value is a list, join it
                if type(value) == list:
                    value = "".join(value).strip()
                else:
                    value = value.strip()

                if properties:
                    output_device["properties"][key] = value
                elif ports:
                    output_device["ports"][key] = value
                elif formats:
                    if key != "":
                        output_device["formats"][key] = value
                else:
                    output_device[key] = value
            
            if number_line == len(output.split("\n")) - 1:
                output_devices.append(output_device)

    return output_devices

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Output audio device')
    parser.add_argument('--debug', action='store_true', help='Debug mode')
    args = parser.parse_args()
    debug = args.debug

    signal.signal(signal.SIGINT, signal.SIG_DFL) # Allow the program to be terminated with Ctrl+C
    main(debug)

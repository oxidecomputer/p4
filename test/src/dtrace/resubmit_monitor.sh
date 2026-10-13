#!/bin/bash

pfexec dtrace -Zq -s "$(dirname "$0")/resubmit_monitor.d"

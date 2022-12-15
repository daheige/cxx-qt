// SPDX-FileCopyrightText: 2022 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>
// SPDX-FileContributor: Andrew Hayzen <andrew.hayzen@kdab.com>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
import QtQuick 2.12
import QtTest 1.12

import com.kdab.cxx_qt.demo 1.0

TestCase {
    name: "ContainerTests"

    Component {
        id: componentContainers

        RustContainers {

        }
    }

    Component {
        id: componentSpy

        SignalSpy {

        }
    }

    function test_container_hash() {
        const obj = createTemporaryObject(componentContainers, null, {});
        const spy = createTemporaryObject(componentSpy, null, {
            signalName: "stringHashChanged",
            target: obj,
        });
        compare(spy.count, 0);
        compare(obj.stringHash, "");

        obj.insertHash("A1", 1);
        obj.insertHash("A1", 1);
        obj.insertHash("A3", 3);
        obj.insertHash("A3", 3);

        compare(spy.count, 4);
        // Order of Hash is not consistent
        verify(obj.stringHash === "A1 => 1, A3 => 3" || obj.stringHash === "A3 => 3, A1 => 1");

        obj.reset();
        compare(spy.count, 5);
        compare(obj.stringHash, "");
    }

    function test_container_set() {
        const obj = createTemporaryObject(componentContainers, null, {});
        const spy = createTemporaryObject(componentSpy, null, {
            signalName: "stringSetChanged",
            target: obj,
        });
        compare(spy.count, 0);
        compare(obj.stringSet, "");

        obj.insertSet(1);
        obj.insertSet(1);
        obj.insertSet(3);
        obj.insertSet(3);

        compare(spy.count, 4);
        // Order of Set is not consistent
        verify(obj.stringSet === "1, 3" || obj.stringSet === "3, 1");

        obj.reset();
        compare(spy.count, 5);
        compare(obj.stringSet, "");
    }
}
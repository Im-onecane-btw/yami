// Прямоугольник со срезанными углами и рамкой (gfx.rs pxbox): заливка, обводка, жёсткая тень.
import QtQuick

Item {
    id: b
    property color fill: "#100c0c"
    property color outline: "transparent"
    property int notch: 2
    property int stroke: 1
    property int shadow: 0
    property bool bevel: false

    // тень — сдвинутая копия формы
    Item {
        visible: b.shadow > 0
        x: b.shadow; y: b.shadow; width: b.width; height: b.height
        Rectangle { x: b.notch; width: parent.width - 2 * b.notch; height: parent.height; color: "#b4000000" }
        Rectangle { y: b.notch; width: parent.width; height: parent.height - 2 * b.notch; color: "#b4000000" }
    }
    Rectangle { x: b.notch; width: b.width - 2 * b.notch; height: b.height; color: b.fill }
    Rectangle { y: b.notch; width: b.notch; height: b.height - 2 * b.notch; color: b.fill }
    Rectangle { x: b.width - b.notch; y: b.notch; width: b.notch; height: b.height - 2 * b.notch; color: b.fill }
    // обводка с уголками-ступеньками
    Item {
        visible: b.outline.a > 0
        anchors.fill: parent
        Rectangle { x: b.notch; width: b.width - 2 * b.notch; height: b.stroke; color: b.outline }
        Rectangle { x: b.notch; y: b.height - b.stroke; width: b.width - 2 * b.notch; height: b.stroke; color: b.outline }
        Rectangle { y: b.notch; width: b.stroke; height: b.height - 2 * b.notch; color: b.outline }
        Rectangle { x: b.width - b.stroke; y: b.notch; width: b.stroke; height: b.height - 2 * b.notch; color: b.outline }
        Rectangle { x: b.stroke; y: b.stroke; width: b.notch - b.stroke; height: b.notch - b.stroke; color: b.outline }
        Rectangle { x: b.width - b.notch; y: b.stroke; width: b.notch - b.stroke; height: b.notch - b.stroke; color: b.outline }
        Rectangle { x: b.stroke; y: b.height - b.notch; width: b.notch - b.stroke; height: b.notch - b.stroke; color: b.outline }
        Rectangle { x: b.width - b.notch; y: b.height - b.notch; width: b.notch - b.stroke; height: b.notch - b.stroke; color: b.outline }
    }
    // фаска: светлая грань сверху-слева, тёмная снизу-справа
    Item {
        visible: b.bevel
        anchors.fill: parent
        Rectangle { x: b.notch; width: b.width - 2 * b.notch; height: 1; color: Qt.lighter(b.fill, 1.35) }
        Rectangle { y: b.notch; width: 1; height: b.height - 2 * b.notch; color: Qt.lighter(b.fill, 1.35) }
        Rectangle { x: b.notch; y: b.height - 1; width: b.width - 2 * b.notch; height: 1; color: Qt.darker(b.fill, 1.8) }
        Rectangle { x: b.width - 1; y: b.notch; width: 1; height: b.height - 2 * b.notch; color: Qt.darker(b.fill, 1.8) }
    }
}

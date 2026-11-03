// Пиксельный текст: Tiny5 рисуется на своей сетке (8 px) и увеличивается целым множителем
// без сглаживания — те же «квадратные» буквы, что в шелле.
import QtQuick

Item {
    id: root
    property alias text: t.text
    property color color: "white"
    /// итоговый размер: 16, 24, 32 … 128 (кратно 8)
    property int px: 16
    property string family: "Tiny5"
    readonly property int k: Math.max(1, Math.round(px / 8))
    width: t.implicitWidth * k
    height: t.implicitHeight * k
    implicitWidth: width
    implicitHeight: height

    Item {
        id: canvas
        width: t.implicitWidth
        height: t.implicitHeight
        layer.enabled: true
        layer.smooth: false
        layer.textureSize: Qt.size(Math.max(1, width), Math.max(1, height))
        transform: Scale { xScale: root.k; yScale: root.k }
        Text {
            id: t
            font.family: root.family
            font.pixelSize: 8
            font.hintingPreference: Font.PreferNoHinting
            color: root.color
            renderType: Text.NativeRendering
            antialiasing: false
        }
    }
}

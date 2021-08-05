#include "cxx-qt-gen/include/my_object.h"
#include "cxx-qt-gen/src/my_object.rs.h"

MyObject::MyObject(QObject* parent)
  : CxxQObject(parent)
  , m_rustObj(createMyObjectRs())
{}

MyObject::~MyObject() = default;

int
MyObject::doubleNumber(int number)
{
  return m_rustObj->doubleNumber(number);
}

QString
MyObject::helloMessage(const QString& msg)
{
  return rustStringToQString(m_rustObj->helloMessage(qStringToRustStr(msg)));
}

QString
MyObject::staticMessage()
{
  return rustStrToQString(m_rustObj->staticMessage());
}

std::unique_ptr<MyObject>
newMyObject()
{
  return std::make_unique<MyObject>();
}

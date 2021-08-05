mod my_object {
    #[cxx::bridge]
    mod ffi {
        unsafe extern "C++" {
            include!("cxx-qt-gen/include/my_object.h");

            type MyObject;
            type QString = cxx_qt_lib::QString;
            type SubObject = crate::sub_object::CppObj;

            #[rust_name = "new_MyObject"]
            fn newMyObject() -> UniquePtr<MyObject>;
        }

        extern "Rust" {
            type MyObjectRs;

            #[cxx_name = "subTest"]
            fn sub_test(self: &MyObjectRs, _cpp: Pin<&mut MyObject>, sub: Pin<&mut SubObject>);

            #[cxx_name = "createMyObjectRs"]
            fn create_my_object_rs() -> Box<MyObjectRs>;
        }
    }

    pub type CppObj = ffi::MyObject;

    struct MyObjectRs;

    impl MyObjectRs {
        fn sub_test(
            &self,
            _cpp: std::pin::Pin<&mut CppObj>,
            sub: std::pin::Pin<&mut crate::sub_object::CppObj>,
        ) {
            println!("Bye from Rust!");
        }
    }

    struct MyObjectWrapper<'a> {
        cpp: std::pin::Pin<&'a mut CppObj>,
    }

    impl<'a> MyObjectWrapper<'a> {
        fn new(cpp: std::pin::Pin<&'a mut CppObj>) -> Self {
            Self { cpp }
        }
    }

    struct MyObjectData;

    fn create_my_object_rs() -> Box<MyObjectRs> {
        Box::new(MyObjectRs {})
    }
}

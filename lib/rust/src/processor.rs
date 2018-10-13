use std::error::Error;

use thrift;
use thrift::protocol::{TInputProtocol, TOutputProtocol};

use context::FContext;
use protocol::{FInputProtocol, FOutputProtocol};

// Adding middleware, processor functions, and annotations is static at startup time (in generated
// code) so probably don't need these explicit here. Maybe have a holder struct. They will be called
// from FXProcesor::new(X, middleware)
//
// TODO:Can I use a macro to construct the inner call that is wrapped by middleware?

pub trait FProcessor {
    fn process(
        &self,
        iprot: &mut FInputProtocol,
        oprot: &mut FOutputProtocol,
    ) -> thrift::Result<()>;

    //fn add_middleware(&mut self, Box<ServiceMiddleware>);

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}

pub trait FProcessorFunction {
    fn process(
        &self,
        ctx: &mut FContext,
        iprot: &mut FInputProtocol,
        oprot: &mut FOutputProtocol,
    ) -> thrift::Result<()>;

    //fn add_middleware(&mut self, Box<ServiceMiddleware>);

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}

pub trait FBaseProcessor {
    fn processor(&self, name: &str) -> Option<&dyn FProcessorFunction>;
    //fn process_map_mut(&mut self) -> BTreeMap<String, Box<FProcessorFunction>>;
    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}

// TODO: This is now in the generated code
impl<T> FProcessor for T
where
    T: FBaseProcessor,
{
    fn process(
        &self,
        iprot: &mut FInputProtocol,
        oprot: &mut FOutputProtocol,
    ) -> thrift::Result<()> {
        let mut ctx = iprot.read_request_header()?;
        let name = iprot.read_message_begin().map(|tmid| tmid.name)?;

        if let Some(processor) = self.processor(&name) {
            match processor.process(&mut ctx, iprot, oprot) {
                Err(thrift::Error::User(err)) => error!("frugal: user handler code returned unhandled error on request with correlation id {}: {}", ctx.correlation_id(), err.description()),
                Err(err) => error!("frugal: user handler code returned unhandled error on request with correlation id {}: {}", ctx.correlation_id(), err.description()),
                _ => (),
            };

            // Return Ok because the server should still send a response to the client.
            return Ok(());
        }

        error!(
            "frugal: client invoked unknown function {} on request with correlation id {}",
            &name,
            ctx.correlation_id()
        );
        iprot.skip(thrift::protocol::TType::Struct)?;
        iprot.read_message_end()?;

        oprot.write_response_header(&ctx)?;
        oprot.write_message_begin(&thrift::protocol::TMessageIdentifier::new(
            &name as &str,
            thrift::protocol::TMessageType::Exception,
            0,
        ))?;
        let ex = thrift::ApplicationError::new(
            thrift::ApplicationErrorKind::UnknownMethod,
            format!("Unknown function {}", &name),
        );
        thrift::Error::write_application_error_to_out_protocol(&ex, oprot)?;
        oprot.write_message_end()?;
        oprot.flush()
    }

    //fn add_middleware(&mut self, Box<ServiceMiddleware>);

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>> {
    //    unimplemented!()
    //}
}

#[cfg(test)]
mod test {
    use std::fs;
    use std::io::{self, Cursor, Read, Write};

    use thrift::protocol::{TBinaryInputProtocolFactory, TBinaryOutputProtocolFactory};

    use super::*;
    use protocol::{FInputProtocolFactory, FOutputProtocolFactory};

    struct MockProcessor(BTreeMap<String, Box<dyn FProcessorFunction>>);

    impl FBaseProcessor for MockProcessor {
        fn process_map(&self) -> &BTreeMap<String, Box<dyn FProcessorFunction>> {
            &self.0
        }
    }

    struct MockProcessorFunction;

    impl FProcessorFunction for MockProcessorFunction {
        fn process(
            &self,
            _: &mut FContext,
            _: &mut FInputProtocol,
            _: &mut FOutputProtocol,
        ) -> thrift::Result<()> {
            Ok(())
        }
    }

    struct WriteMockTransport;
    impl Write for WriteMockTransport {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Ok(0)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    static PING_FRAME: &'static [u8] = &[
        0, 0, 0, 0, 29, 0, 0, 0, 5, 95, 111, 112, 105, 100, 0, 0, 0, 1, 48, 0, 0, 0, 4, 95, 99,
        105, 100, 0, 0, 0, 3, 49, 50, 51, 128, 1, 0, 1, 0, 0, 0, 4, 112, 105, 110, 103, 0, 0, 0, 0,
        2, 0, 1, 1, 0,
    ];

    #[test]
    fn test_f_base_processor_happy_path() {
        let mut process_map = BTreeMap::new();
        process_map.insert(
            "ping".to_string(),
            Box::new(MockProcessorFunction) as Box<dyn FProcessorFunction>,
        );
        let processor = MockProcessor(process_map);
        let input_transport = Box::new(Cursor::new(PING_FRAME));
        let input_prot_factory =
            FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
        let mut iprot = input_prot_factory.get_protocol(input_transport);
        let output_protocol_factory =
            FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
        let output_transport = Box::new(WriteMockTransport);
        let mut oprot = output_protocol_factory.get_protocol(output_transport);

        processor.process(&mut iprot, &mut oprot).unwrap();
    }

    #[test]
    fn test_f_base_processor_error() {
        struct ErrorMockProcessorFunction;

        impl FProcessorFunction for ErrorMockProcessorFunction {
            fn process(
                &self,
                _: &mut FContext,
                _: &mut FInputProtocol,
                _: &mut FOutputProtocol,
            ) -> thrift::Result<()> {
                Err(thrift::new_application_error(
                    thrift::ApplicationErrorKind::Unknown,
                    "error",
                ))
            }
        }
        let mut process_map = BTreeMap::new();
        process_map.insert(
            "ping".to_string(),
            Box::new(ErrorMockProcessorFunction) as Box<dyn FProcessorFunction>,
        );
        let processor = MockProcessor(process_map);
        let input_transport = Box::new(Cursor::new(PING_FRAME));
        let input_prot_factory =
            FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
        let mut iprot = input_prot_factory.get_protocol(input_transport);
        let output_protocol_factory =
            FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
        let output_transport = Box::new(WriteMockTransport);
        let mut oprot = output_protocol_factory.get_protocol(output_transport);

        processor.process(&mut iprot, &mut oprot).unwrap();
    }

    #[test]
    fn test_f_base_processor_read_error() {
        struct ReadMockTransport;
        impl Read for ReadMockTransport {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::ErrorKind::UnexpectedEof.into())
            }
        }
        let mut process_map = BTreeMap::new();
        process_map.insert(
            "ping".to_string(),
            Box::new(MockProcessorFunction) as Box<dyn FProcessorFunction>,
        );
        let processor = MockProcessor(process_map);
        let input_transport = Box::new(ReadMockTransport);
        let input_prot_factory =
            FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
        let mut iprot = input_prot_factory.get_protocol(input_transport);
        let output_protocol_factory =
            FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
        let output_transport = Box::new(WriteMockTransport);
        let mut oprot = output_protocol_factory.get_protocol(output_transport);

        if let Ok(()) = processor.process(&mut iprot, &mut oprot) {
            panic!("process completed successfully, it should not have")
        }
    }

    static F_BASE_PROCESSOR_NO_PROCESSOR_FUNCTION: &'static str =
        "f_base_processor_no_processor_function_file_path.txt";

    #[test]
    fn test_f_base_processor_no_processor_function() {
        let processor = MockProcessor(BTreeMap::new());
        let input_transport = Box::new(Cursor::new(PING_FRAME));
        let input_prot_factory =
            FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
        let mut iprot = input_prot_factory.get_protocol(input_transport);

        {
            let test_file = fs::File::create(F_BASE_PROCESSOR_NO_PROCESSOR_FUNCTION).unwrap();
            let output_protocol_factory =
                FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
            let mut oprot = output_protocol_factory.get_protocol(Box::new(test_file));

            if let Err(err) = processor.process(&mut iprot, &mut oprot) {
                panic!(format!("process errored: {}", err.description()))
            }
        }

        let test_bytes = {
            let mut test_file = fs::File::open(F_BASE_PROCESSOR_NO_PROCESSOR_FUNCTION).unwrap();
            let mut test_vec = Vec::new();
            test_file.read_to_end(&mut test_vec).unwrap();
            test_vec
        };

        let response_ctx: &[u8] = &[
            0u8, 0, 0, 0, 29, 0, 0, 0, 4, 95, 99, 105, 100, 0, 0, 0, 3, 49, 50, 51, 0, 0, 0, 5, 95,
            111, 112, 105, 100, 0, 0, 0, 1, 48,
        ];
        assert_eq!(response_ctx, &test_bytes[..34]);

        assert_eq!("ping".as_bytes(), &test_bytes[42..46]);
        assert_eq!("Unknown function ping".as_bytes(), &test_bytes[57..78]);

        // clean up test file
        fs::remove_file(F_BASE_PROCESSOR_NO_PROCESSOR_FUNCTION).unwrap();
    }
}

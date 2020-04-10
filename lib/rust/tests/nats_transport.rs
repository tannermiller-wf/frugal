// TODO: This test builds and passes but hangs, need to find a way to stop the scheduler I think

//use std::io::Read;
//
//use byteorder::{BigEndian, WriteBytesExt};
//use thrift;
//use thrift::protocol::{
//    TCompactInputProtocolFactory, TCompactOutputProtocolFactory, TOutputProtocol,
//};
//use thrift::transport::{TReadTransport, TWriteTransport};
//use tokio::task;
//
//use frugal::context::FContext;
//use frugal::processor::FProcessor;
//use frugal::protocol::{FInputProtocol, FInputProtocolFactory, FOutputProtocol};
//use frugal::transport::nats::{FNatsServerBuilder, FNatsTransport};
//use frugal::transport::FTransport;
//
//static ADDRESS: &str = "nats://localhost:4222";
//
//fn prepend_frame_size(bs: &[u8]) -> Vec<u8> {
//    let mut frame = Vec::new();
//    frame.write_u32::<BigEndian>(bs.len() as u32).unwrap();
//    frame.extend(bs);
//    frame
//}
//
//#[tokio::test(threaded_scheduler)]
//async fn test_nats_transport_service_integration() {
//    #[derive(Clone)]
//    struct MockProcessor;
//    impl FProcessor for MockProcessor {
//        fn process<R: TReadTransport, W: TWriteTransport>(
//            &self,
//            iprot: &mut FInputProtocol<R>,
//            oprot: &mut FOutputProtocol<W>,
//        ) -> thrift::Result<()> {
//            let ctx = iprot.read_request_header()?;
//            oprot.write_response_header(&ctx)?;
//            let mut proxy = oprot.t_protocol_proxy();
//            proxy.write_string("Hello, World!")?;
//            proxy.flush()
//        }
//    }
//
//
//    let subjects = vec!["testing".to_string()];
//
//    let request_bytes = "Hello from the other side".as_bytes();
//    let framed_request_bytes = prepend_frame_size(request_bytes);
//
//    // set up server in another thread
//    let subjects_clone = subjects.clone();
//    task::spawn(async move {
//        let iprot = FInputProtocolFactory::new(TCompactInputProtocolFactory::new());
//        let oprot =
//            frugal::protocol::FOutputProtocolFactory::new(TCompactOutputProtocolFactory::new());
//        FNatsServerBuilder::new(ADDRESS, MockProcessor, iprot, oprot, subjects_clone)
//            .serve()
//            .await
//            .unwrap();
//    });
//
//    let transport = FNatsTransport::new(ADDRESS, None, "testing", "inbox").unwrap();
//
//    let fctx = FContext::new(None);
//
//    let mut str_from_proc = "".to_string();
//    let out = transport
//        .request(&fctx, &framed_request_bytes)
//        .await
//        .unwrap()
//        .read_to_string(&mut str_from_proc)
//        .unwrap();
//
//    assert_eq!(18, out);
//
//    // use index 5 to skip the protocol version byte and size u32
//    assert_eq!("Hello, World!", &str_from_proc[5..]);
//
//    println!("all done")
//}

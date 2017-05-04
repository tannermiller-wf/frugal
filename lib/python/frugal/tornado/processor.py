import copy
import logging

from thrift.Thrift import TApplicationException
from thrift.Thrift import TException
from thrift.Thrift import TMessageType
from thrift.Thrift import TType
from tornado import gen
from tornado.locks import Lock

from frugal.exceptions import TApplicationExceptionType

logger = logging.getLogger(__name__)


class FProcessorFunction(object):
    """FProcessorFunction is a generic object that exposes a single process
    call, which is used to handle a method invocation. FProcessorFunction
    should be implemented by the generated code."""

    def __init__(self, handler, lock):
        """
            Args:
                handler: frugal.middleware.Method
                lock: tornado.locks.Lock
        """
        self._handler = handler
        self._lock = lock

    @gen.coroutine
    def process(self, ctx, iprot, oprot):
        """Process the request from the input protocol and write the
        response to the output protocol.

        Args:
            iprot: input FProtocol
            oprot: output FProtocol
        """
        pass

    def add_middleware(self, middleware):
        """Add the given middleware to the FProcessorFunction
        This should only be called before the server is started.

            Args:
             middleware: ServiceMiddleware
         """

        self._handler.add_middleware(middleware)


class FProcessor(object):
    """FProcessor is a generic object which operates upon an input stream and
    writes to some output stream.
    """
    @gen.coroutine
    def process(self, iprot, oprot):
        """Process the request from the input protocol and write the
        response to the output protocol.

        Args:
            iprot: input FProtocol
            oprot: output FProtocol
        """
        pass

    def add_middleware(self, middleware):
        """Add the given middleware to the FProcessor
        This should only be called before the server is started.

            Args:
             middleware: ServiceMiddleware
         """
        pass

    def get_annotations_map(self):
        """Return a deepcopy of the annotations map"""
        pass


class FBaseProcessor(FProcessor):
    """FBaseProcessor is a base implementation of FProcessor. FProcessors
    should extend this and map FProcessorFunctions. This should only be used
    by generated code."""

    def __init__(self):
        """Create new instance of FBaseProcessor that will process requests."""
        self._processor_function_map = {}
        self._annotations_map = {}
        self._write_lock = Lock()

    def add_to_processor_map(self, key, proc):
        """Register the given FProcessorFunction.

        Args:
            key: processor function name
            proc: FProcessorFunction
        """
        self._processor_function_map[key] = proc

    def add_to_annotations_map(self, method_name, annotation):
        """Register the given annotation dictionary

        Args:
            method_name: method name
            annotation: annotation dictionary
        """
        self._annotations_map[method_name] = annotation

    def get_annotations_map(self):
        """Return a deepcopy of the annotations map"""
        return copy.deepcopy(self._annotations_map)

    def get_write_lock(self):
        """Return the write lock."""
        return self._write_lock

    @gen.coroutine
    def process(self, iprot, oprot):
        """Process an input protocol and output protocol

        Args:
            iprot: input FProtocol
            oport: ouput FProtocol

        Raises:
            TException: if processing fails.
        """
        context = iprot.read_request_headers()
        name, _, _ = iprot.readMessageBegin()

        processor_function = self._processor_function_map.get(name)

        # If the function was in our dict, call process on it.
        if processor_function:
            try:
                yield processor_function.process(context, iprot, oprot)
            except Exception:
                # Don't raise an exception because the server should still send
                # a response to the client.
                logging.exception(
                    'frugal: exception occurred while processing request with '
                    'correlation id {}'.format(context.correlation_id))
            return

        logger.warn('frugal: client invoked unknown method {0} on request '
                    'with correlation id {1}'.format(
                         name, context.correlation_id))
        iprot.skip(TType.STRUCT)
        iprot.readMessageEnd()

        ex = TApplicationException(
            type=TApplicationExceptionType.UNKNOWN_METHOD,
            message="Unknown function: {0}".format(name))

        with (yield self._write_lock.acquire()):
            oprot.write_response_headers(context)
            oprot.writeMessageBegin(name, TMessageType.EXCEPTION, 0)
            ex.write(oprot)
            oprot.writeMessageEnd()
            oprot.trans.flush()

    def add_middleware(self, middleware):
        """Add the given middleware to the FProcessor.
        This should only be called before the server is started.

        Args:
            middleware: ServiceMiddleware
        """

        if middleware and not isinstance(middleware, list):
            middleware = [middleware]

        for proc in self._processor_function_map.values():
            proc.add_middleware(middleware)

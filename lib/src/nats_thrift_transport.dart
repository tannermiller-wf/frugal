part of frugal;

var codec = new Utf8Codec();

class NatsThriftTransport extends TTransport {
  Nats client;
  String subject;
  Stream<Message> subscription;

  StreamController _signalRead = new StreamController.broadcast();
  Stream get signalRead => _signalRead.stream;

  bool _isOpen;
  final List<int> _writeBuffer = [];
  Iterator<int> _readIterator;


  NatsThriftTransport(this.client);

  Uint8List _consumeWriteBuffer() {
    Uint8List buffer = new Uint8List.fromList(_writeBuffer);
    _writeBuffer.clear();
    return buffer;
  }

  void _setReadBuffer(Uint8List readBuffer) {
    _readIterator = readBuffer != null ? readBuffer.iterator : null;
  }

  void _reset({bool isOpen: false}) {
    _isOpen = isOpen;
    _writeBuffer.clear();
    _readIterator = null;
  }

  bool get hasReadData => _readIterator != null;

  bool get isOpen => subscription != null && _isOpen;

  Future open() async {
    _reset(isOpen: true);
    subscription = await client.subscribe(subject).catchError((e) {
      throw new TTransportError(e);
    });
    subscription.listen((Message msg) {
      _setReadBuffer(msg.payload);
      _signalRead.add(true);
    });
  }

  Future close() async {
    if (isOpen) {
      return new Future.value();
    }
    _reset(isOpen: false);
    await client.unsubscribe(subject);
    subscription = null;
    return super.close();
  }

  int read(Uint8List buffer, int offset, int length) {
    if (buffer == null) {
      throw new ArgumentError.notNull("buffer");
    }

    if (offset + length > buffer.length) {
      throw new ArgumentError("The range exceeds the buffer length");
    }

    if (_readIterator == null || length <= 0) {
      return 0;
    }

    int i = 0;
    while (i < length && _readIterator.moveNext()) {
      buffer[offset + i] = _readIterator.current;
      i++;
    }

    // cleanup iterator when we've reached the end
    if (_readIterator.current == null) {
      _readIterator = null;
    }

    return i;
  }

  void write(Uint8List buffer, int offset, int length) {
    // TODO: Blow up if you go over 1Mb
    if (buffer == null) {
      throw new ArgumentError.notNull("buffer");
    }

    if (offset + length > buffer.length) {
      throw new ArgumentError("The range exceeds the buffer length");
    }

    _writeBuffer.addAll(buffer.sublist(offset, offset + length));
  }

  Future flush() async {
    Uint8List bytes = _consumeWriteBuffer();
    client.publish(subject, "", bytes);
  }

  void setSubject(String subject) {
    this.subject = subject;
  }
}

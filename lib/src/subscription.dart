part of frugal;

class Subscription {
  String subject;
  Transport _transport;
  StreamController _errorControler = new StreamController.broadcast();
  Stream<Error> get error => _errorControler.stream;

  Subscription(this.subject, this._transport);

  /// Unsubscribe from the subject.
  Future unsubscribe() => _transport.unsubscribe();

  signal(Error err) { _errorControler.add(err); }
}
